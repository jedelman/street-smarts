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
//! No courtyard is selected here -- P37's "common land" at block scale is a
//! DIFFERENT thing (informal shared land between clustered buildings) from
//! P95's courtyard (a designed interconnecting space within one building
//! complex) or P61's small square (an intentionally placed public space).
//! Deciding block-level common land is deferred; this operator's only job
//! is giving the rest of the sequence a human-scaled piece of land to work
//! within, instead of the whole site at once.

use crate::p95_building_complex::stratified_seeds;
use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{
    area, bbox, clip_to_polygon, inset_convex, local_to_ring, ring_to_local, union_pieces,
    voronoi_cell, Pt2,
};
use crate::prng::Prng;
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{Neighborhood, Parcel};
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
        let mut steps: Vec<String> = Vec::new();
        let mut prng = Prng::new(seed);
        let mut global_block_idx = 0;

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

            let seeds = stratified_seeds(&local_poly, n_blocks, params.seed_jitter, &mut prng);
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
                    all_new_parcels.push(Parcel {
                        id: format!("{}_BLOCK_{}", parcel_id, global_block_idx),
                        polygon: Polygon::from_ring(block_ring),
                        area_acres: block_area_m2 / 4046.86,
                        use_category: Some("house_cluster_block".into()),
                        ownership: None,
                        is_eda: true,
                        spec: Some(format!("BLOCK_{}", global_block_idx)),
                    });
                    n_emitted += 1;
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

        let trace = SubdivisionTrace {
            operator_name: "p37_house_cluster".into(),
            operator_source: self.source(),
            headline: format!(
                "Carved {} into {} house-cluster block(s), replacing the single raw parcel.",
                parcel_id, all_new_parcels.len()
            ),
            steps,
            caveats: vec![
                "This is land subdivision only -- it does not place a shared common space within \
                 each block. That's a real, undecided design question this operator doesn't \
                 resolve (see the module doc comment).".into(),
                "Random seeding means each reseed produces a different block layout. Block \
                 boundaries from Voronoi cells are geometric, not social -- they don't know about \
                 existing paths of use, sightlines, or where people would actually want the edge \
                 between two clusters to fall.".into(),
                "Blocks are emitted with is_eda=true and no ownership -- downstream operators \
                 (P52, P61, P95) need to run PER BLOCK, targeting each BLOCK_n parcel \
                 individually, not '*'.".into(),
            ],
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels: all_new_parcels,
            new_open_space: vec![],
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
