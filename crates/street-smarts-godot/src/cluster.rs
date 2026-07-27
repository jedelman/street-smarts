//! Restricts a real, pipeline-generated `Neighborhood` down to a small,
//! spatially-compact building cluster -- a fast integration-test fixture
//! that's still real (every building came out of the real pattern
//! pipeline, none of this is synthetic), not a hand-authored fixture that
//! can drift from what the pipeline actually produces.
//!
//! # Why this exists
//! The full real Military Circle site (35 buildings, 3.4M+ triangles) is
//! the right target for a final on-device check, but far too slow for
//! iterating on a single feature: an offscreen software-rendered pass
//! over the whole site took multiple minutes per frame on this dev
//! machine and had to be killed mid-run. A handful of real, adjacent
//! buildings meshes and renders in a small fraction of that time while
//! still exercising the real pipeline end to end -- the progressive
//! integration-test layer between `building_mesh.rs`'s unit tests (which
//! check the geometry math directly, no rendering at all) and a full
//! on-device site walkthrough.

use street_smarts_core::geometry::{LngLat, Ring};
use street_smarts_core::nir::{Building, Neighborhood};

fn ring_centroid(ring: &Ring) -> (f64, f64) {
    if ring.is_empty() {
        return (0.0, 0.0);
    }
    let n = ring.len() as f64;
    let (sum_lng, sum_lat) = ring.iter().fold((0.0, 0.0), |(sx, sy), p: &LngLat| (sx + p.lng, sy + p.lat));
    (sum_lng / n, sum_lat / n)
}

fn dist2(a: (f64, f64), b: (f64, f64)) -> f64 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    dx * dx + dy * dy
}

/// Returns a copy of `nir` whose `buildings` are just `anchor_id` and its
/// `count - 1` nearest other real buildings (by real footprint centroid --
/// plain degree distance, not a proper projection, but at cluster scale
/// [a few hundred meters] the distortion is negligible and this only needs
/// to rank buildings, not measure them). Streets and open spaces are left
/// untouched: ear-clipping over all of them is cheap (see
/// `ground_features.rs`), so the only real cost this needs to cut is
/// per-building Surface Nets extraction.
///
/// Returns `None` if `anchor_id` doesn't name a real building in `nir`, or
/// `count` is `0` -- never silently falls back to the full site or an
/// empty one.
pub fn nearest_building_cluster(nir: &Neighborhood, anchor_id: &str, count: usize) -> Option<Neighborhood> {
    if count == 0 {
        return None;
    }
    let anchor = nir.buildings.iter().find(|b| b.id == anchor_id)?;
    let anchor_center = ring_centroid(&anchor.polygon.outer);

    let mut ranked: Vec<&Building> = nir.buildings.iter().collect();
    ranked.sort_by(|a, b| {
        let da = dist2(ring_centroid(&a.polygon.outer), anchor_center);
        let db = dist2(ring_centroid(&b.polygon.outer), anchor_center);
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut result = nir.clone();
    result.buildings = ranked.into_iter().take(count).cloned().collect();
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;

    fn square_building(id: &str, cx: f64, cy: f64, side: f64) -> Building {
        let h = side / 2.0;
        let outer = vec![
            LngLat::new(cx - h, cy - h),
            LngLat::new(cx + h, cy - h),
            LngLat::new(cx + h, cy + h),
            LngLat::new(cx - h, cy + h),
        ];
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(outer),
            height_m: Some(5.0),
            typology: None,
            year_built: None,
            parcel_id: None,
            floors: None,
            openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
            roof_segments: vec![],
            canopies: vec![],
            wall_niches: vec![],
        }
    }

    fn test_neighborhood(buildings: Vec<Building>) -> Neighborhood {
        use std::collections::HashMap;
        use street_smarts_core::nir::NeighborhoodMeta;
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 1.0, 1.0],
            parcels: vec![],
            buildings,
            streets: vec![],
            open_space: vec![],
            boundaries: vec![],
            activity_nodes: vec![],
            pattern_fields: vec![],
            metadata: NeighborhoodMeta {
                source: "test".into(),
                fetched_at: "2026-01-01".into(),
                license: "test".into(),
                layer_provenance: HashMap::new(),
                label: "test fixture".into(),
            },
        }
    }

    #[test]
    fn keeps_the_anchor_plus_its_real_nearest_neighbors_in_distance_order() {
        // Five buildings on a line, 10 degrees apart -- B2 is the anchor,
        // so the nearest 3 (count=3) should be B2 itself, then B1 and B3
        // (both 10 away), then B0 or B4 (both 20 away) would be next.
        let buildings = vec![
            square_building("B0", 0.0, 0.0, 1.0),
            square_building("B1", 10.0, 0.0, 1.0),
            square_building("B2", 20.0, 0.0, 1.0),
            square_building("B3", 30.0, 0.0, 1.0),
            square_building("B4", 40.0, 0.0, 1.0),
        ];
        let nir = test_neighborhood(buildings);

        let cluster = nearest_building_cluster(&nir, "B2", 3).expect("B2 is a real building");
        let ids: Vec<&str> = cluster.buildings.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&"B2"), "the anchor itself must always be included");
        assert!(ids.contains(&"B1") && ids.contains(&"B3"), "the two real nearest neighbors must be included, got {ids:?}");
    }

    #[test]
    fn unknown_anchor_id_returns_none_not_an_empty_or_full_cluster() {
        let nir = test_neighborhood(vec![square_building("B0", 0.0, 0.0, 1.0)]);
        assert!(nearest_building_cluster(&nir, "does_not_exist", 5).is_none());
    }

    #[test]
    fn zero_count_returns_none() {
        let nir = test_neighborhood(vec![square_building("B0", 0.0, 0.0, 1.0)]);
        assert!(nearest_building_cluster(&nir, "B0", 0).is_none());
    }

    #[test]
    fn count_larger_than_the_site_returns_every_real_building() {
        let nir = test_neighborhood(vec![
            square_building("B0", 0.0, 0.0, 1.0),
            square_building("B1", 5.0, 0.0, 1.0),
        ]);
        let cluster = nearest_building_cluster(&nir, "B0", 100).unwrap();
        assert_eq!(cluster.buildings.len(), 2, "count above the real building total should just cap at the total, not pad or error");
    }
}
