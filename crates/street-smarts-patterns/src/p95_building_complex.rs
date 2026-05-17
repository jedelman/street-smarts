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

use crate::planar::{
    area, bbox, centroid, clip_convex_to_polygon, convex_hull, inset_convex, local_to_ring,
    point_in_polygon, ring_to_local, voronoi_cell, Pt2,
};
use crate::prng::Prng;
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{Neighborhood, OpenSpace, OpenSpaceKind, Parcel};
use street_smarts_core::opinion::SourceCitation;

pub struct P95BuildingComplex;

impl PatternOperator for P95BuildingComplex {
    fn name(&self) -> &'static str { "p95_building_complex" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p95".into(),
            display: "Alexander et al., A Pattern Language, Pattern 95 (Building Complex)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl95/apl95.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Break a monolithic parcel into ~10 building-pad parcels arranged around a courtyard."
    }

    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
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
            let raw_area_m2 = area(&local_poly);

            // Use the convex hull of the part as the working region. This is
            // an honest compromise for v0.1: heavily concave parcels (like
            // MALL_CORE with its U-shape bays) lose those concavities; we
            // subdivide the hull instead. The result is the operator giving
            // its best read on the parcel's "envelope," called out in caveats.
            let work_poly = convex_hull(&local_poly);
            let work_area_m2 = area(&work_poly);
            let hull_ratio = if raw_area_m2 > 0.0 { work_area_m2 / raw_area_m2 } else { 0.0 };
            let part_area_m2 = work_area_m2;
            let part_area_ac = part_area_m2 / 4046.86;

            // Target building count: roughly ~1 per 800 m² for mid-density urban,
            // clamped. (Eyeballed: 26 ac = 105_200 m² → ~10–13 buildings; 1.6 ac
            // = 6_475 m² → ~2 buildings + 1 courtyard.)
            let n_buildings = ((part_area_m2 / 1_000.0).round() as usize).clamp(3, 14);
            steps.push(format!(
                "part[{}] ({:.2} ac, {:.0} m² convex hull, {:.0}% of {:.0} m² raw): targeting {} buildings + 1 courtyard",
                part_idx, part_area_ac, work_area_m2, hull_ratio * 100.0, raw_area_m2, n_buildings
            ));

            // Bounding box of the working polygon.
            let (min_pt, max_pt) = bbox(&work_poly);
            let w = max_pt.x - min_pt.x;
            let h = max_pt.y - min_pt.y;

            // Stratified-random seeding inside the convex hull.
            let target_seeds = n_buildings + 1;
            let seeds = stratified_seeds(&work_poly, target_seeds, &mut prng);
            if seeds.len() < 3 {
                steps.push(format!(
                    "part[{}]: only {} valid seeds (need 3+) — too concave or too small. Skipping.",
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
            let mut cells: Vec<(Pt2, Vec<Pt2>)> = Vec::with_capacity(seeds.len());
            for &site in &seeds {
                let raw = voronoi_cell(site, &seeds, &bound_rect);
                if raw.is_empty() { continue; }
                // Clip against the convex hull. Since work_poly is convex,
                // clip_convex_to_polygon now produces correct results.
                let clipped = clip_convex_to_polygon(&raw, &work_poly);
                if clipped.len() >= 3 && area(&clipped) > 25.0 {
                    cells.push((site, clipped));
                }
            }
            if cells.is_empty() {
                steps.push(format!(
                    "part[{}]: 0 viable cells after clipping — parcel too non-convex for this seed.",
                    part_idx
                ));
                continue;
            }

            // Pick the largest cell as the courtyard.
            let mut cells_sorted = cells.clone();
            cells_sorted.sort_by(|a, b| {
                area(&b.1).partial_cmp(&area(&a.1)).unwrap_or(std::cmp::Ordering::Equal)
            });
            let courtyard = cells_sorted.remove(0);
            steps.push(format!(
                "part[{}]: courtyard = largest cell ({:.0} m²); {} cells become building pads",
                part_idx, area(&courtyard.1), cells_sorted.len()
            ));

            // Emit courtyard as open space (no inset — courtyards fill their cell).
            let courtyard_ring = local_to_ring(&courtyard.1, &origin);
            all_new_open.push(OpenSpace {
                id: format!("{}_P95_courtyard_p{}", parcel_id, part_idx),
                polygon: Polygon::from_ring(courtyard_ring),
                kind: OpenSpaceKind::Plaza,
            });

            // Emit each building-pad cell as a new EDA parcel. Inset by ~3 m
            // to make room for "interconnecting spaces" (paths/alleys).
            for (_, raw_cell) in cells_sorted {
                let inset_cell = inset_convex(&raw_cell, 3.0);
                if inset_cell.len() < 3 || area(&inset_cell) < 60.0 {
                    // Too small after inset — skip rather than emit a sliver.
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
                "v0.1 uses convex-hull approximation when subdividing non-convex parcels. \
                 The generated building pads tile the parcel's CONVEX HULL, not the parcel itself. \
                 On heavily concave parcels (like MALL_CORE with its U-shape bays) this means some \
                 generated pads sit outside the actual parcel boundary. This is a known limitation; \
                 v0.2 will use proper non-convex clipping (ear-decomposition or polygon-clipping crate).".into(),
                "Building pads are abstract polygons. They do not yet include \
                 streets, sidewalks, utilities, frontage rules, or any zoning constraints.".into(),
                "Random seeding means each reseed produces a different layout. Coalition \
                 decisions should not be made from any one variant — the variation IS the chorus.".into(),
            ],
            seed,
        };

        Ok(Subdivision {
            new_parcels: all_new_parcels,
            new_open_space: all_new_open,
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
/// Returns however many we actually got inside (may be less than target if
/// the polygon is concave).
fn stratified_seeds(poly: &[Pt2], target: usize, prng: &mut Prng) -> Vec<Pt2> {
    let (min_pt, max_pt) = bbox(poly);
    let w = max_pt.x - min_pt.x;
    let h = max_pt.y - min_pt.y;
    if w < 1.0 || h < 1.0 { return vec![]; }

    // Stratify into a grid of ~sqrt(target * 1.5) cells; iterate cells in a
    // shuffled order; place one jittered point per cell; accept if inside.
    let grid = ((target as f64 * 1.5).sqrt().ceil() as usize).max(2);
    let cell_w = w / grid as f64;
    let cell_h = h / grid as f64;
    let mut cell_indices: Vec<(usize, usize)> = (0..grid)
        .flat_map(|i| (0..grid).map(move |j| (i, j)))
        .collect();
    // Fisher-Yates shuffle using our PRNG.
    for i in (1..cell_indices.len()).rev() {
        let j = (prng.next_u64() as usize) % (i + 1);
        cell_indices.swap(i, j);
    }

    let mut accepted: Vec<Pt2> = Vec::with_capacity(target);
    let _ = centroid(poly); // unused; reserved for future Lloyd's relaxation
    for (i, j) in cell_indices {
        if accepted.len() >= target { break; }
        // Several jitter attempts per cell, then move on.
        let mut ok = false;
        for _ in 0..6 {
            let x = min_pt.x + (i as f64 + prng.next_f64()) * cell_w;
            let y = min_pt.y + (j as f64 + prng.next_f64()) * cell_h;
            let pt = Pt2::new(x, y);
            if point_in_polygon(pt, poly) {
                // Ensure minimum spacing from existing seeds to avoid bunching.
                let min_dist = (cell_w.min(cell_h)) * 0.5;
                if accepted.iter().all(|&q| q.dist(pt) > min_dist) {
                    accepted.push(pt);
                    ok = true;
                    break;
                }
            }
        }
        let _ = ok;
    }
    accepted
}
