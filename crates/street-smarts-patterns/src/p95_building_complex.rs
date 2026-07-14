//! P95 — Building Complex.
//!
//! From Alexander, *A Pattern Language* (1977), Pattern 95:
//!
//! > Never build large monolithic buildings. Whenever possible translate your
//! > building program into a building complex, whose parts manifest the actual
//! > social facts of the situation. At low densities a building complex may
//! > have its parts; at higher densities a single building can be treated as
//! > a complex, with the parts inside it.
//!
//! This implementation's interpretation:
//!
//! 1. Take a parcel that reads as monolithic (e.g. a dead asphalt mall site).
//! 2. Subtract any land earlier pipeline steps already reserved -- P52 path
//!    corridors, P61 squares -- so seeding happens on what's actually left
//!    to build on, not the raw parcel as if nothing had been decided yet.
//! 3. Seed N building centers inside each remaining buildable piece using
//!    stratified random sampling — N proportional to piece area, capped to
//!    a sensible range.
//! 4. Compute the Voronoi tessellation of those seeds, clipped to the piece.
//! 5. Designate the largest cell as the COURTYARD (open space — the
//!    "interconnecting space" the pattern requires).
//! 6. Inset the remaining cells by ~3m so the buildings don't share walls,
//!    leaving room for the negative-space backbone (paths, alleys).
//! 7. Emit each remaining cell as a new EDA parcel (one proposed building pad).
//!
//! Output: ~10 new building-pad parcels + 1 courtyard open-space per
//! buildable piece, replacing the monolithic source parcel.
//!
//! # v0.2: builds around pre-placed land, doesn't just claim the whole parcel
//!
//! v0.1 always seeded across the full source parcel, regardless of whatever
//! else was already on it. That was fine as long as P95 ran first in the
//! pipeline -- but Alexander's own pattern numbering (52 < 61 < 95) says it
//! shouldn't: P52 (paths) and P61 (small squares) are supposed to be laid
//! down as fixed context BEFORE a building complex reacts to them, not
//! retrofitted into whatever courtyard P95 happened to leave over (see the
//! `pattern_order_prototype.rs` experiment this responds to). This version
//! reads `nbhd.open_space` and `nbhd.streets` for anything already
//! overlapping the target parcel, subtracts it (`planar::subtract_convex`,
//! real polygon subtraction, not a heuristic), and seeds only the buildable
//! remainder. A parcel with nothing pre-placed on it behaves exactly like
//! v0.1 -- this is additive, not a behavior change for the old pipeline order.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{
    area, bbox, centroid, clip_to_polygon, inset_convex, lnglat_to_local, local_to_ring,
    point_in_polygon, rect_corridor, ring_to_local, subtract_convex, union_pieces, voronoi_cell,
    Pt2,
};
use crate::prng::Prng;
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{Neighborhood, OpenSpace, OpenSpaceKind, Parcel};
use street_smarts_core::opinion::SourceCitation;

/// Tunable parameters for P95 Building Complex.
///
/// These are the algorithm's knobs. A future training loop will optimize them
/// against chorus scores; a coalition member can pull them by hand right now.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P95Params {
    /// Target number of building pads per ~1000 m² of parcel area. At 1.0
    /// (default) a 26-acre parcel gets ~14 pads. Pushing this up creates
    /// smaller, denser pads; down creates larger, sparser ones.
    pub buildings_per_kilo_m2: f64,
    /// Minimum pad count regardless of area.
    pub min_buildings: f64,
    /// Maximum pad count regardless of area.
    pub max_buildings: f64,
    /// Inset around each pad in metres. Deliberately tiny (default 0.1m,
    /// a construction joint, not a real setback) -- P95's older 3.0m
    /// default assumed every pad should stand apart from its neighbors,
    /// which is the opposite of what real urban infill (and Alexander's
    /// own P108 Connected Buildings) argues for: buildings running to the
    /// lot line, sharing walls, not each surrounded by its own yard.
    /// `p108_connected_buildings` is what decides which pads should
    /// actually merge into one continuous building and which should stay
    /// separate (a real street or square between them, not just this
    /// inset) -- this parameter no longer does that job on its own.
    pub pad_inset_m: f64,
    /// Stratified-random jitter strength. 0.0 = pure grid, 1.0 = pure random.
    /// (Currently used as a knob on the seeding RNG range.)
    pub seed_jitter: f64,
    /// Minimum pad area in m² after inset. Pieces smaller than this are
    /// discarded as slivers.
    pub min_pad_area_m2: f64,
    /// Minimum fragment area in m² BEFORE inset. Fragments smaller than this
    /// (typically from polygon clipping at concave parcel boundaries) are
    /// discarded before inset.
    pub min_fragment_area_m2: f64,
    /// Minimum bounding-box short side in metres. `min_pad_area_m2` alone
    /// doesn't catch a sliver -- a 54m x 5m strip clears 120 m² easily but
    /// isn't a buildable floor plate at any height; real building footprints
    /// need SOME minimum width regardless of how much area they have.
    /// Dropped the same way an undersized pad is, not shrunk or reshaped.
    pub min_pad_short_side_m: f64,
    /// Courtyard-selection mode (encoded as a float for the param vector):
    /// 0.0 = largest cell becomes courtyard
    /// 1.0 = most-central cell (closest to parcel centroid) becomes courtyard
    /// (intermediate values blend — but for v0.1 we just round to 0 or 1.)
    pub courtyard_mode: f64,
}

impl Parameters for P95Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "buildings_per_kilo_m2",
                "Target pad density. 1.0 ≈ 1 building per ~1000 m² of parcel.",
                0.2, 3.0, 1.0,
            ).with_unit("pads/1000m²"),
            ParamSpec::integer(
                "min_buildings",
                "Minimum pad count regardless of area.",
                1.0, 20.0, 3.0,
            ).with_unit("buildings"),
            ParamSpec::integer(
                "max_buildings",
                "Maximum pad count regardless of area.",
                1.0, 40.0, 14.0,
            ).with_unit("buildings"),
            ParamSpec::float(
                "pad_inset_m",
                "Construction-joint gap between pads, not a real setback -- P108 decides what actually stays separate.",
                0.0, 10.0, 0.1,
            ).with_unit("m"),
            ParamSpec::float(
                "seed_jitter",
                "How randomized the seed placement is. 0=grid-like, 1=pure random.",
                0.0, 1.0, 0.6,
            ),
            ParamSpec::float(
                "min_pad_area_m2",
                "Drop pads smaller than this after inset. Below ~120 m² isn't a building.",
                50.0, 500.0, 120.0,
            ).with_unit("m²"),
            ParamSpec::float(
                "min_fragment_area_m2",
                "Drop polygon-clipping fragments smaller than this before inset.",
                25.0, 300.0, 80.0,
            ).with_unit("m²"),
            ParamSpec::float(
                "min_pad_short_side_m",
                "Drop pads narrower than this on their short bounding-box side, regardless of area -- a real floor plate needs SOME width.",
                3.0, 15.0, 7.0,
            ).with_unit("m"),
            ParamSpec::float(
                "courtyard_mode",
                "0=largest cell becomes courtyard, 1=most-central cell becomes courtyard.",
                0.0, 1.0, 0.0,
            ),
        ]
    }
    fn defaults() -> Self {
        Self {
            buildings_per_kilo_m2: 1.0,
            min_buildings: 3.0,
            max_buildings: 14.0,
            pad_inset_m: 0.1,
            seed_jitter: 0.6,
            min_pad_area_m2: 120.0,
            min_fragment_area_m2: 80.0,
            min_pad_short_side_m: 7.0,
            courtyard_mode: 0.0,
        }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![
            self.buildings_per_kilo_m2,
            self.min_buildings,
            self.max_buildings,
            self.pad_inset_m,
            self.seed_jitter,
            self.min_pad_area_m2,
            self.min_fragment_area_m2,
            self.min_pad_short_side_m,
            self.courtyard_mode,
        ]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(v)) = (schema.get(0), v.get(0)) { p.buildings_per_kilo_m2 = s.clamp(*v); }
        if let (Some(s), Some(v)) = (schema.get(1), v.get(1)) { p.min_buildings = s.clamp(*v); }
        if let (Some(s), Some(v)) = (schema.get(2), v.get(2)) { p.max_buildings = s.clamp(*v); }
        if let (Some(s), Some(v)) = (schema.get(3), v.get(3)) { p.pad_inset_m = s.clamp(*v); }
        if let (Some(s), Some(v)) = (schema.get(4), v.get(4)) { p.seed_jitter = s.clamp(*v); }
        if let (Some(s), Some(v)) = (schema.get(5), v.get(5)) { p.min_pad_area_m2 = s.clamp(*v); }
        if let (Some(s), Some(v)) = (schema.get(6), v.get(6)) { p.min_fragment_area_m2 = s.clamp(*v); }
        if let (Some(s), Some(v)) = (schema.get(7), v.get(7)) { p.min_pad_short_side_m = s.clamp(*v); }
        if let (Some(s), Some(v)) = (schema.get(8), v.get(8)) { p.courtyard_mode = s.clamp(*v); }
        p
    }
}

pub struct P95BuildingComplex;

/// Collect convex "reserved" holes already committed on this part of the
/// parcel by earlier pipeline steps -- existing open space (P37's common
/// land, P61's pre-placed squares) and street rights-of-way -- so P95 seeds
/// pads AROUND them instead of through them. `pub(crate)` so
/// `p61_small_public_squares` can reuse it too, for the same reason: a
/// square shouldn't land on top of common land P37 already reserved on that
/// block.
///
/// Real subtraction (`planar::subtract_convex`) is mathematically exact for
/// a hole that doesn't overlap the subject at all -- it's a no-op -- so this
/// doesn't need to be a precise "is this relevant" filter, just a coarse
/// containment check (hole centroid inside the part) to skip obviously
/// irrelevant entities instead of scanning the whole neighborhood's streets
/// and open space against every part.
///
/// Assumes hole polygons are convex (real squares from P61/a corrected P52
/// stage are; see `subtract_convex`'s own doc comment for the same
/// assumption and its tradeoff). A non-convex existing open-space polygon
/// would subtract its convex hull instead of its real shape -- not guarded
/// against here.
pub(crate) fn reserved_holes_for_part(nbhd: &Neighborhood, local_part: &[Pt2], origin: &LngLat) -> Vec<Vec<Pt2>> {
    let mut holes: Vec<Vec<Pt2>> = Vec::new();

    for o in &nbhd.open_space {
        let local_ring = ring_to_local(&o.polygon.outer, origin);
        if local_ring.len() < 3 {
            continue;
        }
        if point_in_polygon(centroid(&local_ring), local_part) {
            holes.push(local_ring);
        }
    }

    for s in &nbhd.streets {
        if s.centerline.len() < 2 {
            continue;
        }
        let half_width = s.row_width_m.unwrap_or(4.0) / 2.0;
        let local_line: Vec<Pt2> = s.centerline.iter().map(|p| lnglat_to_local(p, origin)).collect();
        for w in local_line.windows(2) {
            let mid = Pt2::new((w[0].x + w[1].x) / 2.0, (w[0].y + w[1].y) / 2.0);
            if point_in_polygon(mid, local_part) {
                let corridor = rect_corridor(w[0], w[1], half_width);
                if corridor.len() >= 3 {
                    holes.push(corridor);
                }
            }
        }
    }

    holes
}

impl PatternOperator for P95BuildingComplex {
    type Params = P95Params;

    fn name(&self) -> &'static str { "p95_building_complex" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p95".into(),
            display: "Alexander et al., A Pattern Language, Pattern 95 (Building Complex)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl95/apl95.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Break a monolithic parcel into N building-pad parcels arranged around a courtyard, building around any land earlier pipeline steps already reserved."
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

        // Anchor projection at the source parcel's outer centroid.
        let origin = LngLat::new(
            average_lng(&source.polygon.outer),
            average_lat(&source.polygon.outer),
        );

        let mut all_new_parcels: Vec<Parcel> = Vec::new();
        let mut all_new_open: Vec<OpenSpace> = Vec::new();
        let mut steps: Vec<String> = Vec::new();
        let mut prng = Prng::new(seed);

        let mut global_cell_idx = 0;
        let mut n_skipped_slivers = 0;

        for (part_idx, part) in parts.iter().enumerate() {
            let raw_local_poly = ring_to_local(&part.outer, &origin);
            if raw_local_poly.len() < 3 {
                steps.push(format!(
                    "part[{}]: skipped (degenerate, only {} pts)",
                    part_idx, raw_local_poly.len()
                ));
                continue;
            }

            // Subtract land earlier pipeline steps already reserved (P52
            // path corridors, P61 squares) before seeding -- build pads
            // around them, not through them. A part with nothing pre-placed
            // on it (the old pipeline order, or a fresh parcel) yields
            // `holes.is_empty()` and behaves exactly like v0.1.
            let holes = reserved_holes_for_part(nbhd, &raw_local_poly, &origin);
            let mut buildable_pieces = vec![raw_local_poly.clone()];
            for hole in &holes {
                // subtract_convex's own output is exact (verified: pieces
                // sum to the correct area, zero hole overlap) -- but do NOT
                // run its pieces through union_pieces expecting it to
                // re-merge them. union_pieces' edge-cancellation assumes
                // triangulation-style splits, where a shared internal edge
                // is the SAME segment in both neighbors. subtract_convex's
                // pieces instead share boundary along the HOLE's cut lines,
                // which real parcel vertices can subdivide differently
                // across neighboring pieces -- cancelling the wrong edges
                // and silently reintroducing part of the hole. Caught this
                // for real: it cost ~300 m² of the "subtracted" square
                // reappearing in a downstream pad. Left un-merged instead;
                // see the piece-count mitigation below.
                buildable_pieces = buildable_pieces
                    .into_iter()
                    .flat_map(|p| subtract_convex(&p, hole))
                    .collect();
            }
            if !holes.is_empty() {
                let reserved_area: f64 = holes.iter().map(|h| area(h)).sum();
                let buildable_area: f64 = buildable_pieces.iter().map(|p| area(p)).sum();
                steps.push(format!(
                    "part[{}]: {} reserved hole(s) from earlier pipeline steps ({:.0} m² claimed) subtracted -- {:.0} m² buildable remains across {} piece(s).",
                    part_idx, holes.len(), reserved_area, buildable_area, buildable_pieces.len()
                ));
            }

            // Leaving subtract_convex's pieces un-merged (see above) means a
            // handful of holes can leave many small buildable fragments, not
            // just the real disjoint regions. `min_fragment_area_m2` (tuned
            // for Voronoi-cell fragments, a much smaller-scale concept) is
            // too low a bar for "does this deserve its own building
            // complex" -- a fragment has to plausibly fit at least
            // min_buildings pads at min_pad_area_m2 each, or it's not a
            // building complex, it's a strip of land next to one.
            let min_worthwhile_area_m2 = params.min_buildings * params.min_pad_area_m2;
            let multi_piece = buildable_pieces.len() > 1;
            let mut n_skipped_small_fragments = 0;
            for (piece_idx, local_poly) in buildable_pieces.into_iter().enumerate() {
                let label = if multi_piece { format!("{part_idx}.{piece_idx}") } else { part_idx.to_string() };
                if local_poly.len() < 3 {
                    continue;
                }
                let part_area_m2 = area(&local_poly);
                if part_area_m2 < min_worthwhile_area_m2 {
                    // Too small to be its own building complex -- not an
                    // error, just a strip of buildable land left over from
                    // subtraction that isn't worth a full seed-and-courtyard
                    // treatment. Counted, not silently dropped.
                    n_skipped_small_fragments += 1;
                    continue;
                }
                let part_area_ac = part_area_m2 / 4046.86;

                // Target building count from params.
                let raw_target = (part_area_m2 / 1_000.0) * params.buildings_per_kilo_m2;
                let n_buildings = (raw_target.round() as usize)
                    .clamp(params.min_buildings as usize, params.max_buildings as usize);
                steps.push(format!(
                    "part[{}] ({:.2} ac, {:.0} m²): targeting {} buildings + 1 courtyard",
                    label, part_area_ac, part_area_m2, n_buildings
                ));

                // Bounding box of the piece.
                let (min_pt, max_pt) = bbox(&local_poly);
                let w = max_pt.x - min_pt.x;
                let h = max_pt.y - min_pt.y;

                // Stratified-random seeding inside the actual buildable polygon.
                let target_seeds = n_buildings + 1;
                let seeds = stratified_seeds(&local_poly, target_seeds, params.seed_jitter, &mut prng);
                if seeds.len() < 2 {
                    steps.push(format!(
                        "part[{}]: only {} valid seeds (need 2+: 1 building + 1 courtyard) — too concave or too small. Skipping.",
                        label, seeds.len()
                    ));
                    continue;
                }
                steps.push(format!(
                    "part[{}]: placed {} seeds (target {})",
                    label, seeds.len(), target_seeds
                ));

                // Voronoi bound = a generous rectangle around the piece bbox.
                let pad = (w + h) * 0.5;
                let bound_rect = vec![
                    Pt2::new(min_pt.x - pad, min_pt.y - pad),
                    Pt2::new(max_pt.x + pad, min_pt.y - pad),
                    Pt2::new(max_pt.x + pad, max_pt.y + pad),
                    Pt2::new(min_pt.x - pad, max_pt.y + pad),
                ];

                // Compute each cell; clip to the piece polygon (lossy for
                // non-convex boundaries — see clip_convex_to_polygon docs).
                // Each Voronoi cell may produce multiple disjoint pieces when
                // clipped to a non-convex boundary. We keep them all
                // (fragmentation is geometric truth, not noise to suppress) but
                // drop pieces too small to plausibly hold a building (< 80 m²).
                let mut cells: Vec<(Pt2, Vec<Pt2>)> = Vec::with_capacity(seeds.len() * 2);
                for &site in &seeds {
                    let raw = voronoi_cell(site, &seeds, &bound_rect);
                    if raw.is_empty() { continue; }
                    let fragments = clip_to_polygon(&raw, &local_poly);
                    // `clip_to_polygon` triangulates the clip boundary and intersects
                    // against each triangle separately -- adjacent fragments from the
                    // SAME site need to be merged back into one pad, or a non-convex
                    // boundary (real building sites usually aren't convex)
                    // shatters one seed into dozens of artificial "pads". Genuinely
                    // disjoint fragments (a seed's cell split by a real concavity)
                    // correctly stay separate.
                    let pieces = union_pieces(&fragments);
                    for piece in pieces {
                        if piece.len() >= 3 && area(&piece) >= params.min_fragment_area_m2 {
                            cells.push((site, piece));
                        }
                    }
                }
                if cells.is_empty() {
                    steps.push(format!(
                        "part[{}]: 0 viable cells after clipping — parcel too non-convex for this seed.",
                        label
                    ));
                    continue;
                }

                // Pick courtyard. Two modes selected via params.courtyard_mode:
                //   < 0.5  → largest cell
                //   ≥ 0.5  → most-central (closest to piece centroid)
                let piece_centroid = centroid(&local_poly);
                let courtyard;
                let cells_sorted: Vec<(Pt2, Vec<Pt2>)>;
                if params.courtyard_mode >= 0.5 {
                    let mut sorted = cells.clone();
                    sorted.sort_by(|a, b| {
                        let da = centroid(&a.1).dist(piece_centroid);
                        let db = centroid(&b.1).dist(piece_centroid);
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    courtyard = sorted.remove(0);
                    cells_sorted = sorted;
                    steps.push(format!(
                        "part[{}]: courtyard = most-central cell ({:.0} m²); {} cells become building pads",
                        label, area(&courtyard.1), cells_sorted.len()
                    ));
                } else {
                    let mut sorted = cells.clone();
                    sorted.sort_by(|a, b| {
                        area(&b.1).partial_cmp(&area(&a.1)).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    courtyard = sorted.remove(0);
                    cells_sorted = sorted;
                    steps.push(format!(
                        "part[{}]: courtyard = largest cell ({:.0} m²); {} cells become building pads",
                        label, area(&courtyard.1), cells_sorted.len()
                    ));
                }

                // Emit courtyard as open space (no inset — courtyards fill their cell).
                let courtyard_ring = local_to_ring(&courtyard.1, &origin);
                all_new_open.push(OpenSpace {
                    id: format!("{}_P95_courtyard_p{}", parcel_id, label),
                    polygon: Polygon::from_ring(courtyard_ring),
                    kind: OpenSpaceKind::Plaza,
                });

                // Emit each building-pad cell as a new EDA parcel. Inset by
                // params.pad_inset_m -- a construction joint now, not a real
                // setback (see the param's own doc comment).
                for (_, raw_cell) in cells_sorted {
                    let inset_cell = if params.pad_inset_m > 0.0 {
                        inset_convex(&raw_cell, params.pad_inset_m)
                    } else {
                        raw_cell
                    };
                    if inset_cell.len() < 3 || area(&inset_cell) < params.min_pad_area_m2 {
                        continue;
                    }
                    // Area alone doesn't catch a sliver -- a long thin strip
                    // clears min_pad_area_m2 easily but isn't a buildable
                    // floor plate at any height (see min_pad_short_side_m's
                    // own doc comment).
                    let (min_pt, max_pt) = bbox(&inset_cell);
                    let short_side = (max_pt.x - min_pt.x).min(max_pt.y - min_pt.y);
                    if short_side < params.min_pad_short_side_m {
                        n_skipped_slivers += 1;
                        continue;
                    }
                    let pad_ring = local_to_ring(&inset_cell, &origin);
                    let pad_area_m2 = area(&inset_cell);
                    let pad_area_ac = pad_area_m2 / 4046.86;
                    global_cell_idx += 1;
                    all_new_parcels.push(Parcel {
                        id: format!("{}_P95_cell_{}", parcel_id, global_cell_idx),
                        polygon: Polygon::from_ring(pad_ring),
                        area_acres: pad_area_ac,
                        use_category: Some("p95_building_pad".into()),
                        ownership: None,
                        is_eda: true,
                        spec: Some(format!("P95_CELL_{}", global_cell_idx)),
                        // Carry the source block's P29 density assignment
                        // forward so P96 Number of Stories has a tier to
                        // work from without needing to re-derive which
                        // block a pad came from.
                        density_tier: source.density_tier.clone(),
                        target_stories: source.target_stories,
                    });
                }
            }
            if n_skipped_small_fragments > 0 {
                steps.push(format!(
                    "part[{}]: {} buildable fragment(s) from subtraction were smaller than min_buildings×min_pad_area_m2 ({:.0} m²) -- not worth a full building complex, left as unclaimed land rather than forced into one.",
                    part_idx, n_skipped_small_fragments, min_worthwhile_area_m2
                ));
            }
        }

        if n_skipped_slivers > 0 {
            steps.push(format!(
                "{} pad(s) cleared min_pad_area_m2 but were narrower than min_pad_short_side_m ({:.1}m) on their short side -- dropped as slivers, not stretched or shrunk into a usable shape.",
                n_skipped_slivers, params.min_pad_short_side_m
            ));
        }

        if all_new_parcels.is_empty() && all_new_open.is_empty() {
            return Err(format!(
                "P95 produced no output for parcel {} (all parts too non-convex, too small, or fully reserved by earlier steps)",
                parcel_id
            ));
        }

        let trace = SubdivisionTrace {
            operator_name: "p95_building_complex".into(),
            operator_source: self.source(),
            headline: format!(
                "Subdivided {} into {} building-pad parcel{} around {} courtyard{}.",
                parcel_id,
                all_new_parcels.len(),
                if all_new_parcels.len() == 1 { "" } else { "s" },
                all_new_open.len(),
                if all_new_open.len() == 1 { "" } else { "s" },
            ),
            steps,
            caveats: vec![
                "This subdivision was auto-generated. It is NOT a coalition proposal. \
                 It is one algorithm's interpretation of one Alexander pattern, with a random seed.".into(),
                "Building pads are abstract polygons. They do not yet include \
                 streets, sidewalks, utilities, frontage rules, or any zoning constraints. \
                 Interior building footprints, daylight wings, entrance placement, and courtyard \
                 articulation happen in downstream operators.".into(),
                "Random seeding means each reseed produces a different layout. Coalition \
                 decisions should not be made from any one variant — the variation IS the chorus.".into(),
                "v0.1 uses Voronoi cells as a coarse first-pass at building positions. Real building \
                 design at this stage would also account for solar orientation, prevailing wind, \
                 view-sheds, existing trees, and the felt edges of the parcel. None of those are inputs here.".into(),
                "Reserved-land subtraction (existing open space / streets) assumes those holes are \
                 convex -- true for the squares and path corridors this codebase's own operators \
                 produce, not guaranteed for hand-authored or third-party fixtures.".into(),
                "min_pad_short_side_m drops a pad based on its bounding-box short side, not its \
                 true minimum width along every direction -- a non-convex or diagonal sliver could \
                 still slip through with a short bbox side that overstates its narrowest point.".into(),
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

/// Place `target` seed points inside `poly` using stratified random sampling.
/// `jitter_strength` controls the randomness within each cell: 0.0 = always
/// place at cell center (grid-like); 1.0 = place uniformly within the cell.
/// Returns however many we actually got inside (may be less than target if
/// the polygon is concave).
///
/// `pub`, not private: the pattern-order prototype (see
/// `tests/pattern_order_prototype.rs`) reuses this to seed path/square nodes
/// directly on raw parcel land, before any P95 pad-carving runs.
pub fn stratified_seeds(poly: &[Pt2], target: usize, jitter_strength: f64, prng: &mut Prng) -> Vec<Pt2> {
    let (min_pt, max_pt) = bbox(poly);
    let w = max_pt.x - min_pt.x;
    let h = max_pt.y - min_pt.y;
    if w < 1.0 || h < 1.0 { return vec![]; }

    let grid = ((target as f64 * 1.5).sqrt().ceil() as usize).max(2);
    let cell_w = w / grid as f64;
    let cell_h = h / grid as f64;
    let mut cell_indices: Vec<(usize, usize)> = (0..grid)
        .flat_map(|i| (0..grid).map(move |j| (i, j)))
        .collect();
    for i in (1..cell_indices.len()).rev() {
        let j = (prng.next_u64() as usize) % (i + 1);
        cell_indices.swap(i, j);
    }

    let jitter = jitter_strength.clamp(0.0, 1.0);
    let half = 0.5 * (1.0 - jitter);
    let mut accepted: Vec<Pt2> = Vec::with_capacity(target);
    for (i, j) in cell_indices {
        if accepted.len() >= target { break; }
        let mut placed = false;
        for _ in 0..6 {
            // Interpolate between cell-center (jitter=0) and full-cell (jitter=1).
            let fx = half + prng.next_f64() * (1.0 - 2.0 * half);
            let fy = half + prng.next_f64() * (1.0 - 2.0 * half);
            let x = min_pt.x + (i as f64 + fx) * cell_w;
            let y = min_pt.y + (j as f64 + fy) * cell_h;
            let pt = Pt2::new(x, y);
            if point_in_polygon(pt, poly) {
                let min_dist = (cell_w.min(cell_h)) * 0.5;
                if accepted.iter().all(|&q| q.dist(pt) > min_dist) {
                    accepted.push(pt);
                    placed = true;
                    break;
                }
            }
        }
        let _ = placed;
    }
    accepted
}
