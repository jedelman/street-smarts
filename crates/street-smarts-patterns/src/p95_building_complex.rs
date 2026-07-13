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
//! 2. Seed N building centers inside the parcel using stratified random
//!    sampling — N proportional to parcel area, capped to a sensible range.
//! 3. Compute the Voronoi tessellation of those seeds, clipped to the parcel.
//! 4. Designate the largest cell as the COURTYARD (open space — the
//!    "interconnecting space" the pattern requires).
//! 5. Inset the remaining cells by ~3m so the buildings don't share walls,
//!    leaving room for the negative-space backbone (paths, alleys).
//! 6. Emit each remaining cell as a new EDA parcel (one proposed building pad).
//!
//! Output: ~10 new building-pad parcels + 1 courtyard open-space, replacing
//! the monolithic source parcel.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{
    area, bbox, centroid, clip_to_polygon, inset_convex, local_to_ring,
    point_in_polygon, ring_to_local, union_pieces, voronoi_cell, Pt2,
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
    /// Inset around each pad in metres — the "interconnecting space" of P95
    /// (paths, alleys, shared yards between buildings).
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
                "Inset around each pad — width of paths/alleys between buildings.",
                0.0, 10.0, 3.0,
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
            pad_inset_m: 3.0,
            seed_jitter: 0.6,
            min_pad_area_m2: 120.0,
            min_fragment_area_m2: 80.0,
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
        if let (Some(s), Some(v)) = (schema.get(7), v.get(7)) { p.courtyard_mode = s.clamp(*v); }
        p
    }
}

pub struct P95BuildingComplex;

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
        "Break a monolithic parcel into N building-pad parcels arranged around a courtyard."
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

        for (part_idx, part) in parts.iter().enumerate() {
            let local_poly = ring_to_local(&part.outer, &origin);
            if local_poly.len() < 3 {
                steps.push(format!(
                    "part[{}]: skipped (degenerate, only {} pts)",
                    part_idx, local_poly.len()
                ));
                continue;
            }
            let part_area_m2 = area(&local_poly);
            let part_area_ac = part_area_m2 / 4046.86;

            // Target building count from params.
            let raw_target = (part_area_m2 / 1_000.0) * params.buildings_per_kilo_m2;
            let n_buildings = (raw_target.round() as usize)
                .clamp(params.min_buildings as usize, params.max_buildings as usize);
            steps.push(format!(
                "part[{}] ({:.2} ac, {:.0} m²): targeting {} buildings + 1 courtyard",
                part_idx, part_area_ac, part_area_m2, n_buildings
            ));

            // Bounding box of the part.
            let (min_pt, max_pt) = bbox(&local_poly);
            let w = max_pt.x - min_pt.x;
            let h = max_pt.y - min_pt.y;

            // Stratified-random seeding inside the actual parcel polygon.
            let target_seeds = n_buildings + 1;
            let seeds = stratified_seeds(&local_poly, target_seeds, params.seed_jitter, &mut prng);
            if seeds.len() < 2 {
                steps.push(format!(
                    "part[{}]: only {} valid seeds (need 2+: 1 building + 1 courtyard) — too concave or too small. Skipping.",
                    part_idx, seeds.len()
                ));
                continue;
            }
            steps.push(format!(
                "part[{}]: placed {} seeds (target {})",
                part_idx, seeds.len(), target_seeds
            ));

            // Voronoi bound = a generous rectangle around the part bbox.
            let pad = (w + h) * 0.5;
            let bound_rect = vec![
                Pt2::new(min_pt.x - pad, min_pt.y - pad),
                Pt2::new(max_pt.x + pad, min_pt.y - pad),
                Pt2::new(max_pt.x + pad, max_pt.y + pad),
                Pt2::new(min_pt.x - pad, max_pt.y + pad),
            ];

            // Compute each cell; clip to the part polygon (lossy for non-convex
            // parcels — see clip_convex_to_polygon docs).
            // Each Voronoi cell may produce multiple disjoint pieces when
            // clipped to a non-convex parcel boundary. We keep them all
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
                // parcel boundary (real building sites usually aren't convex)
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
                    part_idx
                ));
                continue;
            }

            // Pick courtyard. Two modes selected via params.courtyard_mode:
            //   < 0.5  → largest cell
            //   ≥ 0.5  → most-central (closest to part centroid)
            let part_centroid = centroid(&local_poly);
            let courtyard;
            let cells_sorted: Vec<(Pt2, Vec<Pt2>)>;
            if params.courtyard_mode >= 0.5 {
                let mut sorted = cells.clone();
                sorted.sort_by(|a, b| {
                    let da = centroid(&a.1).dist(part_centroid);
                    let db = centroid(&b.1).dist(part_centroid);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                });
                courtyard = sorted.remove(0);
                cells_sorted = sorted;
                steps.push(format!(
                    "part[{}]: courtyard = most-central cell ({:.0} m²); {} cells become building pads",
                    part_idx, area(&courtyard.1), cells_sorted.len()
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
                    part_idx, area(&courtyard.1), cells_sorted.len()
                ));
            }

            // Emit courtyard as open space (no inset — courtyards fill their cell).
            let courtyard_ring = local_to_ring(&courtyard.1, &origin);
            all_new_open.push(OpenSpace {
                id: format!("{}_P95_courtyard_p{}", parcel_id, part_idx),
                polygon: Polygon::from_ring(courtyard_ring),
                kind: OpenSpaceKind::Plaza,
            });

            // Emit each building-pad cell as a new EDA parcel. Inset by
            // params.pad_inset_m to make room for "interconnecting spaces".
            for (_, raw_cell) in cells_sorted {
                let inset_cell = if params.pad_inset_m > 0.0 {
                    inset_convex(&raw_cell, params.pad_inset_m)
                } else {
                    raw_cell
                };
                if inset_cell.len() < 3 || area(&inset_cell) < params.min_pad_area_m2 {
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
                });
            }
        }

        if all_new_parcels.is_empty() && all_new_open.is_empty() {
            return Err(format!(
                "P95 produced no output for parcel {} (all parts too non-convex or too small)",
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
fn stratified_seeds(poly: &[Pt2], target: usize, jitter_strength: f64, prng: &mut Prng) -> Vec<Pt2> {
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
