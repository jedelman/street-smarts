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
//! # A real, checkable shape-complexity proxy
//!
//! This schema has no material/furnishing data ("places to sit, lean")
//! to check directly. What IS real: a footprint's perimeter relative to
//! its area is a standard shape-complexity measure -- a perfect square
//! (or circle) minimizes perimeter for its area; any real crenelation
//! (recesses, projections, a courtyard ring) increases it. This opinion
//! computes `shape_index = perimeter / (4 * sqrt(area))` per building --
//! exactly `1.0` for a perfect square, greater than `1.0` for anything
//! with real edge articulation -- and scores the fraction meeting
//! `min_shape_index` (default 1.15, a 15% perimeter premium over the
//! simplest possible shape for that area).
//!
//! Expected finding on real output, not asserted in advance: P107's
//! `p107_solid_v01` inscribed-rectangle buildings should score near 1.0
//! (a simple rectangle, no real edge complexity), while
//! `p107_courtyard_v01` buildings should score well above threshold
//! (the courtyard hole substantially increases perimeter-per-area) --
//! though incidentally, not because anything currently designs real
//! "places to stop" at the edge.

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
            details.insert(format!("{}.shape_index", b.id), format!("{shape_index:.2}"));
            if shape_index < MIN_SHAPE_INDEX {
                flat_edged.push(b.id.clone());
            }
        }

        if shape_indices.is_empty() {
            return OpinionOutput::NoView {
                reason: "No building had positive real footprint area to score.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let value = shape_indices.iter().filter(|s| **s >= MIN_SHAPE_INDEX).count() as f64 / shape_indices.len() as f64;
        let mean_shape_index = shape_indices.iter().sum::<f64>() / shape_indices.len() as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("meets_threshold_fraction".into(), value);
        sub_scores.insert("mean_shape_index".into(), mean_shape_index);

        details.insert("n_buildings".into(), shape_indices.len().to_string());
        details.insert("min_shape_index_threshold".into(), format!("{MIN_SHAPE_INDEX:.2}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} building(s); mean shape index {:.2} (1.0 = perfect square, no edge \
                 complexity); {:.0}% exceed the {:.2} threshold.",
                shape_indices.len(), mean_shape_index, value * 100.0, MIN_SHAPE_INDEX
            ),
            sub_scores,
            details,
            caveats: vec![
                "shape_index is a real geometric complexity proxy (perimeter / area), not a check \
                 for actual usable edge depth, seating, or shelter -- no furnishing/material data \
                 exists in this schema to check Alexander's literal 'places to sit, lean' \
                 requirement.".into(),
                "A courtyard building's ring shape inflates this score incidentally (the hole adds \
                 real perimeter) even though nothing currently designs a real crenelated edge \
                 zone -- a high score here doesn't confirm Alexander's actual intent was met.".into(),
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
        }
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
        }
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
