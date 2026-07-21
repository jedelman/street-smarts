//! P120 Paths and Goals — a path should connect real destinations at both
//! ends, not run between two arbitrary points.
//!
//! From Alexander, *A Pattern Language*, Pattern 120, via
//! patternlanguage.cc/Patterns/Paths-and-Goals-(120):
//! > **Solution:** To lay out paths, first place goals at natural points
//! > of interest. Then connect the goals to one another to form the
//! > paths.
//!
//! # A real, checkable proxy
//!
//! For each street, this opinion checks whether BOTH of its endpoints sit
//! within `goal_threshold_m` (default 15m) of a real "goal" -- a building
//! centroid, or an open-space feature's centroid (a plaza, park, or
//! common land). A path with a real goal at both ends is doing what
//! Alexander describes; one that dangles at either end (or both) is a
//! path built without a goal to reach.
//!
//! `value` = fraction of streets with a real goal at BOTH ends.

use std::collections::BTreeMap;
use street_smarts_core::geometry::{haversine_m, LngLat};
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P120PathsAndGoals;

const GOAL_THRESHOLD_M: f64 = 15.0;

impl Opinion for P120PathsAndGoals {
    fn name(&self) -> &'static str {
        "p120_paths_and_goals"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p120".into(),
            display: "Alexander et al., A Pattern Language, Pattern 120 (Paths and Goals)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Paths-and-Goals-(120)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        if n.streets.is_empty() {
            return OpinionOutput::NoView {
                reason: "No streets in this neighborhood.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let goals: Vec<LngLat> = n.buildings.iter().map(|b| b.polygon.centroid())
            .chain(n.open_space.iter().filter(|o| o.polygon.area_m2() > 0.0).map(|o| o.polygon.centroid()))
            .collect();
        if goals.is_empty() {
            return OpinionOutput::NoView {
                reason: "No real buildings or open space to serve as goals yet.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut both_ends: Vec<bool> = Vec::new();
        let mut dangling: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for s in &n.streets {
            if s.centerline.len() < 2 {
                continue;
            }
            let a = s.centerline.first().unwrap();
            let b = s.centerline.last().unwrap();
            let a_has_goal = goals.iter().any(|g| haversine_m(a, g) <= GOAL_THRESHOLD_M);
            let b_has_goal = goals.iter().any(|g| haversine_m(b, g) <= GOAL_THRESHOLD_M);
            let ok = a_has_goal && b_has_goal;
            both_ends.push(ok);
            details.insert(format!("{}.start_has_goal", s.id), a_has_goal.to_string());
            details.insert(format!("{}.end_has_goal", s.id), b_has_goal.to_string());
            if !ok {
                dangling.push(s.id.clone());
            }
        }

        if both_ends.is_empty() {
            return OpinionOutput::NoView {
                reason: "No street had two real endpoints to check.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let value = both_ends.iter().filter(|b| **b).count() as f64 / both_ends.len() as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("both_ends_have_goals".into(), value);
        details.insert("n_streets".into(), both_ends.len().to_string());
        details.insert("goal_threshold_m".into(), format!("{GOAL_THRESHOLD_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} street(s); {:.0}% have a real goal (building or open space) within {:.0}m of \
                 BOTH endpoints.",
                both_ends.len(), value * 100.0, GOAL_THRESHOLD_M
            ),
            sub_scores,
            details,
            caveats: vec![
                "'Goal' is any nearby building or open-space centroid -- doesn't confirm the path \
                 was actually laid out FROM that goal (Alexander's stated design sequence), only \
                 that one happens to be nearby now.".into(),
                "A street connecting two block centroids (path_network's own real construction) \
                 will often pass this check by the mechanics of how it was built, not because \
                 anyone identified real goals first.".into(),
            ],
            contributing_features: dangling,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{Building, NeighborhoodMeta, Street};

    fn nbhd(buildings: Vec<Building>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings, streets, open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P120 unit fixture".into(),
            },
        }
    }

    fn building_at(id: &str, x_m: f64, y_m: f64) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![LngLat::new(x_m * m, y_m * m), LngLat::new((x_m + 5.0) * m, y_m * m), LngLat::new((x_m + 5.0) * m, (y_m + 5.0) * m), LngLat::new(x_m * m, (y_m + 5.0) * m), LngLat::new(x_m * m, y_m * m)]),
            height_m: Some(6.0), typology: Some("p107_solid_v01".into()), year_built: None, parcel_id: None, floors: Some(2), openings: vec![], interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        }
    }

    fn street(id: &str, a: (f64, f64), b: (f64, f64)) -> Street {
        let m = 1.0 / 111_320.0;
        Street { id: id.into(), centerline: vec![LngLat::new(a.0 * m, a.1 * m), LngLat::new(b.0 * m, b.1 * m)], classification: Some("local".into()), row_width_m: Some(4.0), surface: None }
    }

    #[test]
    fn no_streets_is_no_view() {
        assert!(matches!(P120PathsAndGoals.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_street_between_two_real_buildings_passes() {
        let n = nbhd(vec![building_at("B1", 0.0, 0.0), building_at("B2", 100.0, 0.0)], vec![street("S1", (2.0, 2.0), (102.0, 2.0))]);
        let out = P120PathsAndGoals.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_street_dangling_in_open_land_fails() {
        let n = nbhd(vec![building_at("B1", 0.0, 0.0)], vec![street("S1", (2.0, 2.0), (5000.0, 2.0))]);
        let out = P120PathsAndGoals.evaluate(&n);
        match out {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-6);
                assert!(contributing_features.contains(&"S1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
