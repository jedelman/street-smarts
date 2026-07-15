//! P37 House Cluster — carve raw land into blocks BEFORE anything else runs.
//!
//! From Alexander, *A Pattern Language*, Pattern 37:
//! > People will not feel that they have a stake in their own house cluster,
//! > or take responsibility for it, unless the cluster itself is a distinct,
//! > identifiable place, separated from other clusters by common land.
//!
//! # Why this exists
//! `block_grouping.rs` already cites this pattern (alongside P14/P36) but
//! groups pads that ALREADY EXIST -- it clusters P95's output after the
//! fact, for PathNetwork's benefit. That's a real operator with a real
//! job, but it can't fix the actual gap: nothing carves the site into
//! human-scaled clusters BEFORE P95 seeds buildings. Running P95 once
//! across a 47-acre raw parcel produces ~100 pads in one flat Voronoi
//! field -- no big/small contrast anywhere, one thin path threading
//! through a uniform mesh. That's not a parameter to tune; Alexander's own
//! sequence has an intermediate-scale pattern here and this pipeline
//! didn't have an operator for it. This is that operator: it carves the
//! RAW parcel into block-scale sub-parcels FIRST, using the same
//! Voronoi-seed-and-clip machinery `P95BuildingComplex` already uses for
//! building pads, just at 10-20x coarser granularity (blocks, not pads).
//!
//! Emits blocks tagged `spec = "BLOCK_<n>"` -- the SAME convention
//! `PathNetwork` already looks for (`spec.starts_with("BLOCK_")`), so it
//! connects these blocks with zero changes on its end. `block_grouping.rs`
//! becomes unused once this runs first in a corrected pipeline (it has
//! nothing left to group -- there's no flat pad soup to cluster after the
//! fact) but isn't removed; it's still a real operator for pipelines that
//! don't use this one.
//!
//! # v0.2: common land, closing the gap above
//!
//! v0.1 deferred block-level common land entirely. This version reserves
//! one: after carving each block, a `common_land_fraction` (default 12%)
//! slice of that block's own area is scaled inward from the block's
//! footprint toward its centroid (`planar::scale_toward_centroid`) and
//! emitted as `OpenSpaceKind::Common` -- a real, distinct kind from
//! `Plaza` (P61's intentional, publicly-scaled square) or a P95 courtyard
//! (a designed space within one building complex). This is the SAME
//! architecture P61's raw-land placement already uses: the block `Parcel`
//! itself is emitted unchanged (still the full carved footprint), and
//! downstream P95 picks the common land up via its existing reserved-land
//! subtraction (`reserved_holes_for_part` scans ALL of `nbhd.open_space`,
//! not just `Plaza`-kind) -- no P95 changes were needed for this.
//!
//! Skipped when the target area falls below `min_common_land_area_m2`
//! (mirrors P61's own `min_meaningful_area_m2` reasoning) -- a tiny
//! detached-annex block that only barely survives its own inset shouldn't
//! also lose more of itself to a common-land patch nobody could use.
//!
//! `p61_small_public_squares::place_new_squares_n` was updated at the same
//! time to subtract whatever common land P37 already placed on a block
//! before scattering its own squares there, so the two don't overlap when
//! a block happens to get both.
//!
//! # What this still does NOT do
//! - Placement is a centered, scaled-down copy of the block's own shape --
//!   an honest first approximation (same category of simplification as
//!   P61's grid partition), not Alexander's real intent that common land
//!   be what the cluster's houses actually face onto, shaped by real
//!   entrances and sightlines this operator has no model of.
//! - The common-land fraction is uniform across all blocks. Real house
//!   clusters vary in how much shared land they set aside; this doesn't
//!   respond to block shape, adjacency, or anything but its own area.
//!
//! # v0.3: field-guided seeding (prototype, opt-in)
//!
//! `seeding_mode` (0=Stratified, the unchanged v0.2 default; 1=FieldGuided)
//! is a narrow port of eastside-commons' `EC_FieldSolver` idea: instead of
//! a blind jittered grid, block seeds are placed at local maxima of a
//! rasterized pressure field built from real anchors already present in
//! `nbhd` -- a positive Gaussian bump at every parcel whose `spec` starts
//! with `"CIVIC"` (eastside-commons' own tagging convention for civic
//! anchors, e.g. `CIVIC_700`), and positive pressure along every street
//! centerline in `nbhd.streets`. The intuition: house clusters in real
//! neighborhoods aren't scattered at random -- they cluster near existing
//! civic anchors and along existing streets, the same way EC's pattern
//! defs paint pressure toward `SPINE`/`MAP` anchors. See `field.rs` for
//! the ported math and for what was deliberately NOT ported (EC's
//! line-tracing and bounding-rect footprints -- street-smarts keeps its
//! own, more rigorous Voronoi-cell footprint step regardless of seeding
//! mode). Falls back to `Stratified` automatically when the parcel has no
//! CIVIC-tagged neighbors and no streets to pull toward -- a raw
//! greenfield site with literally nothing to converge on.

use crate::field::Field;
use crate::p95_building_complex::stratified_seeds;
use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{
    area, average_centroid, bbox, clip_to_polygon, inset_convex, lnglat_to_local, local_to_ring,
    ring_to_local, scale_toward_centroid, union_pieces, voronoi_cell, Pt2,
};
use crate::prng::Prng;
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{Neighborhood, OpenSpace, OpenSpaceKind, Parcel};
use street_smarts_core::opinion::SourceCitation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P37Params {
    /// Target block area in m². Alexander's own House Cluster examples run
    /// roughly 8-12 households sharing common land; at redevelopment scale
    /// (~500m² per eventual building pad, per P95's own default) that's
    /// very roughly 0.6-1 hectare (1.5-2.5 acres) per cluster. Default
    /// splits the difference.
    pub target_block_area_m2: f64,
    /// Minimum block count regardless of area (a tiny parcel still gets
    /// split into at least this many, if it can).
    pub min_blocks: f64,
    /// Maximum block count regardless of area.
    pub max_blocks: f64,
    /// Gap between blocks in metres -- real streets, wider than P95's
    /// pad_inset_m (alleys between buildings within one complex). This is
    /// the right-of-way PathNetwork's connectors will run through.
    pub block_inset_m: f64,
    /// Stratified-random jitter strength, same meaning as P95's seed_jitter.
    pub seed_jitter: f64,
    /// Minimum block area in m² after inset. Blocks smaller than this are
    /// discarded as slivers.
    pub min_block_area_m2: f64,
    /// Fraction of each block's own area reserved as informal common land
    /// (Alexander's "common land" that identifies the cluster) -- scaled
    /// inward from the block's footprint toward its centroid. 0 disables
    /// common-land generation entirely.
    pub common_land_fraction: f64,
    /// Skip common-land generation for a block if the resulting patch
    /// would be smaller than this -- not worth reserving land nobody could
    /// use as shared space.
    pub min_common_land_area_m2: f64,
    /// Block-seeding strategy, float-encoded like P95's `courtyard_mode`:
    /// 0=Stratified (blind jittered grid, the v0.2 default), 1=FieldGuided
    /// (prototype -- seeds pulled toward CIVIC-tagged parcels and street
    /// centerlines already in the neighborhood; see the v0.3 module doc).
    pub seeding_mode: f64,
}

impl Parameters for P37Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "target_block_area_m2",
                "Target land area per house-cluster block.",
                2000.0, 20000.0, 7000.0,
            ).with_unit("m²"),
            ParamSpec::integer(
                "min_blocks",
                "Minimum block count regardless of area.",
                1.0, 10.0, 2.0,
            ).with_unit("blocks"),
            ParamSpec::integer(
                "max_blocks",
                "Maximum block count regardless of area.",
                1.0, 30.0, 12.0,
            ).with_unit("blocks"),
            ParamSpec::float(
                "block_inset_m",
                "Right-of-way between blocks -- real streets, wider than pad-to-pad alleys.",
                4.0, 20.0, 10.0,
            ).with_unit("m"),
            ParamSpec::float(
                "seed_jitter",
                "How randomized block seed placement is. 0=grid-like, 1=pure random.",
                0.0, 1.0, 0.5,
            ),
            ParamSpec::float(
                "min_block_area_m2",
                "Drop blocks smaller than this after inset.",
                500.0, 5000.0, 1500.0,
            ).with_unit("m²"),
            ParamSpec::float(
                "common_land_fraction",
                "Fraction of each block reserved as informal common land (0 disables it).",
                0.0, 0.4, 0.12,
            ),
            ParamSpec::float(
                "min_common_land_area_m2",
                "Skip common-land generation for a block if the patch would be smaller than this.",
                50.0, 1000.0, 150.0,
            ).with_unit("m²"),
            ParamSpec::float(
                "seeding_mode",
                "Block-seeding strategy: 0=Stratified (blind jittered grid), 1=FieldGuided (prototype -- seeds pulled toward civic anchors and streets).",
                0.0, 1.0, 0.0,
            ),
        ]
    }
    fn defaults() -> Self {
        Self {
            target_block_area_m2: 7000.0,
            min_blocks: 2.0,
            max_blocks: 12.0,
            block_inset_m: 10.0,
            seed_jitter: 0.5,
            min_block_area_m2: 1500.0,
            common_land_fraction: 0.12,
            min_common_land_area_m2: 150.0,
            seeding_mode: 0.0,
        }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![
            self.target_block_area_m2,
            self.min_blocks,
            self.max_blocks,
            self.block_inset_m,
            self.seed_jitter,
            self.min_block_area_m2,
            self.common_land_fraction,
            self.min_common_land_area_m2,
            self.seeding_mode,
        ]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.target_block_area_m2 = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.min_blocks = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) { p.max_blocks = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(3), v.get(3)) { p.block_inset_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(4), v.get(4)) { p.seed_jitter = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(5), v.get(5)) { p.min_block_area_m2 = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(6), v.get(6)) { p.common_land_fraction = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(7), v.get(7)) { p.min_common_land_area_m2 = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(8), v.get(8)) { p.seeding_mode = s.clamp(*x); }
        p
    }
}

pub struct P37HouseCluster;

impl PatternOperator for P37HouseCluster {
    type Params = P37Params;

    fn name(&self) -> &'static str { "p37_house_cluster" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p37".into(),
            display: "Alexander et al., A Pattern Language, Pattern 37 (House Cluster)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl37/apl37.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Carve a raw parcel into human-scaled BLOCK_n sub-parcels before any building or square is placed."
    }

    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        let source = nbhd
            .parcels
            .iter()
            .find(|p| p.id == parcel_id)
            .ok_or_else(|| format!("parcel {} not found", parcel_id))?;

        let parts = source.polygon.parts_view();
        if parts.is_empty() {
            return Err("source parcel has no geometry parts".into());
        }

        let origin = LngLat::new(
            average_lng(&source.polygon.outer),
            average_lat(&source.polygon.outer),
        );

        let mut all_new_parcels: Vec<Parcel> = Vec::new();
        let mut all_new_open: Vec<OpenSpace> = Vec::new();
        let mut steps: Vec<String> = Vec::new();
        let mut prng = Prng::new(seed);
        let mut global_block_idx = 0;
        let mut n_common_land_emitted = 0;
        let mut n_common_land_skipped_small = 0;

        for (part_idx, part) in parts.iter().enumerate() {
            let local_poly = ring_to_local(&part.outer, &origin);
            if local_poly.len() < 3 {
                steps.push(format!("part[{}]: skipped (degenerate, only {} pts)", part_idx, local_poly.len()));
                continue;
            }
            let part_area_m2 = area(&local_poly);
            let part_area_ac = part_area_m2 / 4046.86;

            // min_blocks is a floor for parts big enough to plausibly hold
            // that many -- it shouldn't force a small standalone part (a
            // detached annex, say) to split into pieces too small to
            // survive the inset. A part smaller than one target block just
            // becomes ONE block, not min_blocks worth of doomed fragments.
            let raw_target = part_area_m2 / params.target_block_area_m2;
            let n_blocks = if raw_target < 1.0 {
                1
            } else {
                (raw_target.round() as usize).clamp(params.min_blocks as usize, params.max_blocks as usize)
            };
            steps.push(format!(
                "part[{}] ({:.2} ac, {:.0} m²): targeting {} block(s) of ~{:.0} m² each",
                part_idx, part_area_ac, part_area_m2, n_blocks, params.target_block_area_m2
            ));

            let seeds = if params.seeding_mode >= 0.5 {
                let result = field_guided_seeds(nbhd, &local_poly, &origin, n_blocks, &mut prng);
                steps.push(format!(
                    "part[{}]: field-guided seeding -- {} civic anchor(s), {} street segment(s), {} seed(s) from field maxima{}",
                    part_idx, result.n_civic_anchors, result.n_street_segments, result.n_field_seeds,
                    if result.n_field_seeds < n_blocks { ", remainder filled by stratified seeding" } else { "" }
                ));
                if result.seeds.is_empty() {
                    steps.push(format!("part[{}]: no civic anchors or streets nearby -- falling back to stratified seeding", part_idx));
                    stratified_seeds(&local_poly, n_blocks, params.seed_jitter, &mut prng)
                } else {
                    result.seeds
                }
            } else {
                stratified_seeds(&local_poly, n_blocks, params.seed_jitter, &mut prng)
            };
            if seeds.len() < 1 {
                steps.push(format!("part[{}]: 0 valid seeds -- too small or too concave. Skipping.", part_idx));
                continue;
            }
            steps.push(format!("part[{}]: placed {} block seed(s) (target {})", part_idx, seeds.len(), n_blocks));

            let (min_pt, max_pt) = bbox(&local_poly);
            let w = max_pt.x - min_pt.x;
            let h = max_pt.y - min_pt.y;
            let pad = (w + h) * 0.5;
            let bound_rect = vec![
                Pt2::new(min_pt.x - pad, min_pt.y - pad),
                Pt2::new(max_pt.x + pad, min_pt.y - pad),
                Pt2::new(max_pt.x + pad, max_pt.y + pad),
                Pt2::new(min_pt.x - pad, max_pt.y + pad),
            ];

            let mut n_emitted = 0;
            let mut n_dropped_small = 0;
            for &site in &seeds {
                let raw = voronoi_cell(site, &seeds, &bound_rect);
                if raw.is_empty() { continue; }
                let fragments = clip_to_polygon(&raw, &local_poly);
                // Same reasoning as P95: merge same-site triangulation
                // fragments back together (this IS the correct merge case
                // for union_pieces -- these come from one convex cell
                // triangle-clipped, not from subtract_convex's cut-line
                // decomposition; see planar.rs's warning on that distinction).
                for piece in union_pieces(&fragments) {
                    if piece.len() < 3 { continue; }
                    // Inset makes room for a real street BETWEEN sibling
                    // blocks -- a part with only one block (seeds.len()==1)
                    // has no neighbor to leave that gap for, so insetting it
                    // would only shrink a small standalone parcel (a
                    // detached annex, say) for no reason. No inset when
                    // there's nothing to make room next to.
                    let inset = if params.block_inset_m > 0.0 && seeds.len() > 1 {
                        inset_convex(&piece, params.block_inset_m)
                    } else {
                        piece
                    };
                    if inset.len() < 3 || area(&inset) < params.min_block_area_m2 {
                        n_dropped_small += 1;
                        continue;
                    }
                    let block_ring = local_to_ring(&inset, &origin);
                    let block_area_m2 = area(&inset);
                    global_block_idx += 1;
                    let block_id = format!("{}_BLOCK_{}", parcel_id, global_block_idx);
                    all_new_parcels.push(Parcel {
                        id: block_id.clone(),
                        polygon: Polygon::from_ring(block_ring),
                        area_acres: block_area_m2 / 4046.86,
                        use_category: Some("house_cluster_block".into()),
                        ownership: None,
                        is_eda: true,
                        spec: Some(format!("BLOCK_{}", global_block_idx)),
                        // Set later by P29 Density Rings, if it runs.
                        density_tier: None,
                        target_stories: None,
                    });
                    n_emitted += 1;

                    // Common land: Alexander's "distinct, identifiable
                    // place" a cluster's households share. Scaled inward
                    // from the block's own footprint toward its centroid --
                    // does NOT touch the block Parcel just emitted above,
                    // same architecture P61's raw-land squares use. P95
                    // picks this up later via its existing reserved-land
                    // subtraction (scans ALL open_space, not just Plaza).
                    if params.common_land_fraction > 0.0 {
                        let target_area = block_area_m2 * params.common_land_fraction;
                        if target_area >= params.min_common_land_area_m2 {
                            let factor = params.common_land_fraction.sqrt();
                            let common_local = scale_toward_centroid(&inset, factor);
                            if common_local.len() >= 3 {
                                all_new_open.push(OpenSpace {
                                    id: format!("{block_id}_common"),
                                    polygon: Polygon::from_ring(local_to_ring(&common_local, &origin)),
                                    kind: OpenSpaceKind::Common,
                                });
                                n_common_land_emitted += 1;
                            }
                        } else {
                            n_common_land_skipped_small += 1;
                        }
                    }
                }
            }
            steps.push(format!(
                "part[{}]: emitted {} block(s), dropped {} fragment(s) too small after inset",
                part_idx, n_emitted, n_dropped_small
            ));
        }

        if all_new_parcels.is_empty() {
            return Err(format!(
                "P37 produced no blocks for parcel {} (all parts too small, too concave, or fully consumed by inset)",
                parcel_id
            ));
        }

        steps.push(format!(
            "common land: {} block(s) got a {:.0}%-of-area common-land patch, {} skipped (below {:.0} m²).",
            n_common_land_emitted, params.common_land_fraction * 100.0, n_common_land_skipped_small, params.min_common_land_area_m2
        ));

        let trace = SubdivisionTrace {
            operator_name: "p37_house_cluster".into(),
            operator_source: self.source(),
            headline: format!(
                "Carved {} into {} house-cluster block(s) ({} with common land), replacing the single raw parcel.",
                parcel_id, all_new_parcels.len(), n_common_land_emitted
            ),
            steps,
            caveats: vec![
                "Common land is a centered, scaled-down copy of each block's own shape -- an \
                 honest first approximation, not Alexander's real intent that it be shaped by \
                 where the cluster's houses actually face and enter. See the module doc comment's \
                 v0.2 section.".into(),
                "Random seeding means each reseed produces a different block layout. Block \
                 boundaries from Voronoi cells are geometric, not social -- they don't know about \
                 existing paths of use, sightlines, or where people would actually want the edge \
                 between two clusters to fall.".into(),
                "Blocks are emitted with is_eda=true and no ownership -- downstream operators \
                 (P52, P61, P95) need to run PER BLOCK, targeting each BLOCK_n parcel \
                 individually, not '*'.".into(),
                "seeding_mode=FieldGuided is a v0.3 prototype: seeds cluster toward CIVIC-tagged \
                 parcels and streets already in the neighborhood instead of a blind jittered grid, \
                 but the pressure-field math (sigma, weights, threshold) hasn't been tuned against \
                 real outcomes the way Stratified's parameters have -- treat its output as a \
                 hypothesis to compare against Stratified, not a default.".into(),
            ],
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels: all_new_parcels,
            new_open_space: all_new_open,
            new_buildings: vec![],
            new_streets: vec![],
            replaced_parcel_ids: vec![parcel_id.to_string()],
            replaced_open_space_ids: vec![],
            trace,
        })
    }
}

fn average_lng(ring: &[LngLat]) -> f64 {
    if ring.is_empty() { return 0.0; }
    ring.iter().map(|p| p.lng).sum::<f64>() / ring.len() as f64
}
fn average_lat(ring: &[LngLat]) -> f64 {
    if ring.is_empty() { return 0.0; }
    ring.iter().map(|p| p.lat).sum::<f64>() / ring.len() as f64
}

struct FieldSeedResult {
    seeds: Vec<Pt2>,
    n_civic_anchors: usize,
    n_street_segments: usize,
    n_field_seeds: usize,
}

/// v0.3 prototype: place block seeds at local maxima of a pressure field
/// built from real anchors in `nbhd` -- see the module doc's v0.3 section
/// and `field.rs` for what's actually being ported from EC_FieldSolver.
fn field_guided_seeds(
    nbhd: &Neighborhood,
    local_poly: &[Pt2],
    origin: &LngLat,
    target: usize,
    prng: &mut Prng,
) -> FieldSeedResult {
    if target == 0 || local_poly.len() < 3 {
        return FieldSeedResult { seeds: vec![], n_civic_anchors: 0, n_street_segments: 0, n_field_seeds: 0 };
    }

    let (min_pt, max_pt) = bbox(local_poly);
    let w = (max_pt.x - min_pt.x).max(1.0);
    let h = (max_pt.y - min_pt.y).max(1.0);
    // Pad the field beyond the polygon's own bbox so a civic anchor or
    // street just outside the block-carving parcel still pulls seeds
    // toward that edge, not just anchors strictly inside it.
    let pad = w.max(h) * 0.15;
    let field_min = Pt2::new(min_pt.x - pad, min_pt.y - pad);
    let field_max = Pt2::new(max_pt.x + pad, max_pt.y + pad);
    let diag = ((w + 2.0 * pad).powi(2) + (h + 2.0 * pad).powi(2)).sqrt();
    // Aim for roughly 80 cells across the diagonal -- fine enough to find
    // real local maxima, coarse enough to stay fast on a large site.
    let cell_size = (diag / 80.0).max(3.0);

    let mut field = Field::new(field_min, field_max, cell_size);
    let mut n_civic_anchors = 0;
    let mut n_street_segments = 0;

    for p in &nbhd.parcels {
        if p.spec.as_deref().map(|s| s.starts_with("CIVIC")).unwrap_or(false) {
            let anchor_lnglat = average_centroid(&p.polygon.outer);
            let anchor_local = lnglat_to_local(&anchor_lnglat, origin);
            field.paint_gaussian(anchor_local, 45.0, 1.0);
            n_civic_anchors += 1;
        }
    }
    for street in &nbhd.streets {
        for pair in street.centerline.windows(2) {
            let a = lnglat_to_local(&pair[0], origin);
            let b = lnglat_to_local(&pair[1], origin);
            field.paint_segment(a, b, 18.0, 0.6);
            n_street_segments += 1;
        }
    }

    if n_civic_anchors == 0 && n_street_segments == 0 {
        return FieldSeedResult { seeds: vec![], n_civic_anchors, n_street_segments, n_field_seeds: 0 };
    }
    field.normalize();

    let poly_area = area(local_poly);
    let min_separation = (poly_area / target as f64).sqrt() * 0.6;
    let field_seeds = field.find_seeds(local_poly, target, 0.12, min_separation);
    let n_field_seeds = field_seeds.len();

    let mut seeds = field_seeds;
    if seeds.len() < target {
        // The field tells us where pressure IS, not where the parcel still
        // has usable room once target exceeds the number of real anchors
        // -- fill remaining slots with stratified seeding, rejecting any
        // candidate that would crowd a seed the field already placed.
        let extra = stratified_seeds(local_poly, target * 2, 0.6, prng);
        for p in extra {
            if seeds.len() >= target { break; }
            if seeds.iter().all(|&q| q.dist(p) >= min_separation * 0.6) {
                seeds.push(p);
            }
        }
    }

    FieldSeedResult { seeds, n_civic_anchors, n_street_segments, n_field_seeds }
}
