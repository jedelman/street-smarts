//! P160 Building Edge — a building's perimeter should be a real zone with
//! depth (crenelated, places to stop), not a flat line with no thickness.
//!
//! From Alexander, *A Pattern Language*, Pattern 160 (p. 752), via
//! patternlanguage.cc/Patterns/Building-Edge-(160):
//! > **Problem:** A building is most often thought of as something which
//! > turns inward... People do not often think of a building as something
//! > which must also be oriented toward the outside.
//! > **Solution:** Treat the edge of the building as a 'thing', a
//! > 'place', a zone with volume to it, not a line or interface which has
//! > no thickness. Crenelate the edge of buildings with places that
//! > invite people to stop...
//!
//! # A real, checkable shape-complexity proxy -- now backed by real data
//! # too, when it exists
//!
//! This schema had no material/furnishing data ("places to sit, lean")
//! to check directly, so this opinion originally fell back entirely on a
//! geometric proxy: a footprint's perimeter relative to its area is a
//! standard shape-complexity measure -- a perfect square (or circle)
//! minimizes perimeter for its area; any real crenelation (recesses,
//! projections, a courtyard ring) increases it. `shape_index = perimeter
//! / (4 * sqrt(area))` per building -- exactly `1.0` for a perfect
//! square, greater than `1.0` for anything with real edge articulation --
//! scored against `min_shape_index` (default 1.15, a 15% perimeter
//! premium over the simplest possible shape for that area).
//!
//! `Building.wall_niches` (`Vec<WallNiche>`, a real local bulge in an
//! exterior wall's own depth) now exists, specifically for this
//! pattern's own literal claim ("deep enough to contain seats,
//! bookshelves, bay windows"). No generator populates it yet, so on
//! every real fixture this pipeline ships today `wall_niches` is empty
//! everywhere and this opinion still falls back to the shape-index
//! proxy exactly as before. When a real niche DOES exist on a building
//! (verified against synthetic fixtures in this file's own tests), it's
//! treated as direct evidence and satisfies this opinion regardless of
//! shape_index -- real local depth beats a geometric proxy for it.
//!
//! Expected finding on real output, not asserted in advance: P107's
//! `p107_solid_v01` inscribed-rectangle buildings should score near 1.0
//! shape_index (a simple rectangle, no real edge complexity), while
//! `p107_courtyard_v01` buildings should score well above threshold
//! (the courtyard hole substantially increases perimeter-per-area) --
//! though incidentally, not because anything currently designs real
//! "places to stop" at the edge (that's what `wall_niches` is for, once
//! a generator exists).

use std::collections::BTreeMap;
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P160BuildingEdge;

const MIN_SHAPE_INDEX: f64 = 1.15;

fn ring_perimeter_m(ring: &[street_smarts_core::geometry::LngLat]) -> f64 {
    if ring.len() < 2 {
        return 0.0;
    }
    ring.windows(2).map(|w| haversine_m(&w[0], &w[1])).sum()
}

impl Opinion for P160BuildingEdge {
    fn name(&self) -> &'static str {
        "p160_building_edge"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p160".into(),
            display: "Alexander et al., A Pattern Language, Pattern 160 (Building Edge)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Building-Edge-(160)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        if n.buildings.is_empty() {
            return OpinionOutput::NoView {
                reason: "No buildings in this neighborhood.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut shape_indices: Vec<f64> = Vec::new();
        let mut n_ok = 0usize;
        let mut n_real_niche = 0usize;
        let mut flat_edged: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for b in &n.buildings {
            let area = b.polygon.area_m2();
            if area <= 0.0 {
                continue;
            }
            // Include the courtyard hole's own perimeter, if any -- a real
            // ring adds real edge (both an inner AND outer face), which is
            // exactly the kind of edge-articulation this pattern rewards.
            let mut perimeter = ring_perimeter_m(&b.polygon.outer);
            for part in b.polygon.parts_view() {
                for hole in &part.holes {
                    perimeter += ring_perimeter_m(hole);
                }
            }
            let shape_index = perimeter / (4.0 * area.sqrt());
            shape_indices.push(shape_index);
            // Real local depth (a real WallNiche, see this file's own module
            // doc) is direct evidence and beats the geometric proxy -- a
            // building can satisfy this pattern either way.
            let has_real_niche = !b.wall_niches.is_empty();
            let ok = shape_index >= MIN_SHAPE_INDEX || has_real_niche;
            details.insert(format!("{}.shape_index", b.id), format!("{shape_index:.2}"));
            details.insert(format!("{}.n_wall_niches", b.id), b.wall_niches.len().to_string());
            if has_real_niche {
                n_real_niche += 1;
            }
            if ok {
                n_ok += 1;
            } else {
                flat_edged.push(b.id.clone());
            }
        }

        if shape_indices.is_empty() {
            return OpinionOutput::NoView {
                reason: "No building had positive real footprint area to score.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let value = n_ok as f64 / shape_indices.len() as f64;
        let mean_shape_index = shape_indices.iter().sum::<f64>() / shape_indices.len() as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("meets_threshold_fraction".into(), value);
        sub_scores.insert("mean_shape_index".into(), mean_shape_index);

        details.insert("n_buildings".into(), shape_indices.len().to_string());
        details.insert("min_shape_index_threshold".into(), format!("{MIN_SHAPE_INDEX:.2}"));
        details.insert("n_buildings_with_real_wall_niches".into(), n_real_niche.to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} building(s); mean shape index {:.2} (1.0 = perfect square, no edge \
                 complexity); {:.0}% meet the {:.2} threshold or carry a real wall niche \
                 ({} building(s) have one).",
                shape_indices.len(), mean_shape_index, value * 100.0, MIN_SHAPE_INDEX, n_real_niche
            ),
            sub_scores,
            details,
            caveats: vec![
                "shape_index is a real geometric complexity proxy (perimeter / area), not a check \
                 for actual usable edge depth, seating, or shelter -- it's the fallback signal when \
                 no real WallNiche exists. No real fixture this pipeline ships today produces a \
                 WallNiche (no generator populates it yet), so this opinion currently scores on the \
                 proxy alone in practice.".into(),
                "A courtyard building's ring shape inflates the proxy incidentally (the hole adds \
                 real perimeter) even though nothing currently designs a real crenelated edge \
                 zone -- a high shape_index alone doesn't confirm Alexander's actual intent was \
                 met; a real WallNiche is stronger, direct evidence when present.".into(),
            ],
            contributing_features: flat_edged,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon, PolygonPart};
    use street_smarts_core::nir::{Building, NeighborhoodMeta};

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P160 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn m() -> f64 { 1.0 / 111_320.0 }

    fn square_building(id: &str, side_m: f64) -> Building {
        let mm = m();
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0), LngLat::new(side_m * mm, 0.0),
                LngLat::new(side_m * mm, side_m * mm), LngLat::new(0.0, side_m * mm), LngLat::new(0.0, 0.0),
            ]),
            height_m: Some(9.0), typology: Some("p107_solid_v01".into()),
            year_built: None, parcel_id: None, floors: Some(3), openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], }
    }

    fn courtyard_building(id: &str, outer_half: f64, hole_half: f64) -> Building {
        let mm = m();
        let ring = |half: f64| vec![
            LngLat::new(-half * mm, -half * mm), LngLat::new(half * mm, -half * mm),
            LngLat::new(half * mm, half * mm), LngLat::new(-half * mm, half * mm), LngLat::new(-half * mm, -half * mm),
        ];
        Building {
            id: id.into(),
            polygon: Polygon::from_parts(vec![PolygonPart { outer: ring(outer_half), holes: vec![ring(hole_half)] }]),
            height_m: Some(9.0), typology: Some("p107_courtyard_v01".into()),
            year_built: None, parcel_id: None, floors: Some(3), openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], }
    }

    #[test]
    fn no_buildings_is_no_view() {
        assert!(matches!(P160BuildingEdge.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_simple_square_building_scores_near_1_shape_index_and_fails_threshold() {
        let n = nbhd(vec![square_building("B1", 20.0)]);
        let out = P160BuildingEdge.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, contributing_features, .. } => {
                assert!((sub_scores["mean_shape_index"] - 1.0).abs() < 0.02, "got {}", sub_scores["mean_shape_index"]);
                assert!(contributing_features.contains(&"B1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_courtyard_ring_has_a_much_higher_shape_index() {
        let n = nbhd(vec![courtyard_building("CY1", 20.0, 8.0)]);
        let out = P160BuildingEdge.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!(sub_scores["mean_shape_index"] > MIN_SHAPE_INDEX, "got {}", sub_scores["mean_shape_index"]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_real_wall_niche_satisfies_even_a_plain_square_building() {
        use street_smarts_core::nir::WallNiche;
        let mut b = square_building("NICHED", 20.0);
        b.wall_niches.push(WallNiche { ring_index: 0, on_hole: false, t_start: 0.3, t_end: 0.5, extra_depth_m: 0.5 });
        let n = nbhd(vec![b]);
        match P160BuildingEdge.evaluate(&n) {
            OpinionOutput::Value { value, sub_scores, .. } => {
                assert!((sub_scores["mean_shape_index"] - 1.0).abs() < 0.02, "shape_index should still be near 1.0");
                assert!((value - 1.0).abs() < 1e-9, "a real niche should satisfy despite a low shape_index, got {value}");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn mixed_solid_and_courtyard_buildings_split_the_threshold_fraction() {
        let n = nbhd(vec![square_building("SOLID1", 20.0), courtyard_building("CY1", 20.0, 8.0)]);
        let out = P160BuildingEdge.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => {
                assert!((value - 0.5).abs() < 1e-6, "exactly one of two buildings should meet threshold, got {value}");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
