//! P99 Main Building — a complex needs one legible, dominant building at
//! its center, not a uniform crowd of equals.
//!
//! From Alexander, *A Pattern Language*, Pattern 99, via
//! patternlanguage.cc/Patterns/Main-Building-(99):
//! > **Problem:** A complex of buildings with no center is like a person
//! > without a head.
//! > **Solution:** For any collection of buildings, decide which building
//! > in the group houses the most essential function... Form this
//! > building as the main building, with a central position, higher roof.
//!
//! # Two real, checkable proxies
//!
//! This schema has no "essential function" or program data -- which
//! building is the institution's "soul" can't be checked. What IS real:
//! Alexander's own operational cues, height and position.
//!
//! - `height_dominance`: whether the tallest building is meaningfully
//!   taller than the rest (`>= dominance_ratio`, default 1.3x, the mean
//!   height of every OTHER building) -- a proxy for "higher roof."
//! - `centrality`: how close the tallest building's centroid is to the
//!   area-weighted centroid of every building, normalized against the
//!   site's own real spread (the mean distance from that centroid to
//!   every building) -- `1.0` if the tallest building sits exactly at
//!   the center, falling toward `0.0` as it sits as far out as a typical
//!   building does.
//!
//! `value = 0.5 * height_dominance + 0.5 * centrality`. Needs 2+ real
//! buildings to say anything.

use std::collections::BTreeMap;
use street_smarts_core::geometry::{haversine_m, LngLat};
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P99MainBuilding;

const DOMINANCE_RATIO: f64 = 1.3;

impl Opinion for P99MainBuilding {
    fn name(&self) -> &'static str {
        "p99_main_building"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p99".into(),
            display: "Alexander et al., A Pattern Language, Pattern 99 (Main Building)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Main-Building-(99)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let with_height: Vec<(&str, f64, LngLat)> = n.buildings.iter()
            .filter_map(|b| Some((b.id.as_str(), b.height_m?, b.polygon.centroid())))
            .collect();
        if with_height.len() < 2 {
            return OpinionOutput::NoView {
                reason: format!("{} building(s) with a known height -- need at least two to compare.", with_height.len()),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let (tallest_id, tallest_h, tallest_pos) = *with_height.iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
        let others: Vec<f64> = with_height.iter().filter(|(id, _, _)| *id != tallest_id).map(|(_, h, _)| *h).collect();
        let mean_other_height = others.iter().sum::<f64>() / others.len() as f64;
        let height_dominance = if mean_other_height > 0.0 {
            ((tallest_h / mean_other_height - 1.0) / (DOMINANCE_RATIO - 1.0)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let (mut cx, mut cy) = (0.0, 0.0);
        for (_, _, pos) in &with_height {
            cx += pos.lng;
            cy += pos.lat;
        }
        let center = LngLat::new(cx / with_height.len() as f64, cy / with_height.len() as f64);
        let mean_spread = with_height.iter().map(|(_, _, p)| haversine_m(&center, p)).sum::<f64>() / with_height.len() as f64;
        let tallest_dist = haversine_m(&center, &tallest_pos);
        let centrality = if mean_spread > 0.0 {
            (1.0 - tallest_dist / mean_spread).clamp(0.0, 1.0)
        } else {
            1.0
        };

        let value = 0.5 * height_dominance + 0.5 * centrality;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("height_dominance".into(), height_dominance);
        sub_scores.insert("centrality".into(), centrality);

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_buildings".into(), with_height.len().to_string());
        details.insert("tallest_building".into(), tallest_id.to_string());
        details.insert("tallest_height_m".into(), format!("{tallest_h:.1}"));
        details.insert("mean_other_height_m".into(), format!("{mean_other_height:.1}"));
        details.insert("tallest_dist_from_center_m".into(), format!("{tallest_dist:.0}"));
        details.insert("mean_dist_from_center_m".into(), format!("{mean_spread:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} building(s); tallest is {} at {:.1}m (mean of the rest: {:.1}m, dominance \
                 {:.2}); it sits {:.0}m from the group's centroid (mean building distance: {:.0}m, \
                 centrality {:.2}).",
                with_height.len(), tallest_id, tallest_h, mean_other_height, height_dominance,
                tallest_dist, mean_spread, centrality
            ),
            sub_scores,
            details,
            caveats: vec![
                "Cannot check 'houses the most essential function' at all -- no use/program data \
                 exists in this schema. Height and centrality are Alexander's own operational cues, \
                 used as proxies for the underlying institutional-importance judgment, not a \
                 substitute for it.".into(),
                "Assumes the TALLEST building is the intended main building -- if a real design \
                 puts the main building's importance in massing or ornament rather than height, \
                 this opinion would miss it entirely.".into(),
            ],
            contributing_features: vec![tallest_id.to_string()],
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{Building, NeighborhoodMeta};

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.02, 0.02],
            parcels: vec![], buildings, streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P99 unit fixture".into(),
            },
        }
    }

    fn building_at(id: &str, x_m: f64, y_m: f64, height_m: f64) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(x_m * m, y_m * m), LngLat::new((x_m + 10.0) * m, y_m * m),
                LngLat::new((x_m + 10.0) * m, (y_m + 10.0) * m), LngLat::new(x_m * m, (y_m + 10.0) * m),
                LngLat::new(x_m * m, y_m * m),
            ]),
            height_m: Some(height_m), typology: Some("p107_solid_v01".into()),
            year_built: None, parcel_id: None, floors: None, openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], }
    }

    #[test]
    fn fewer_than_two_buildings_is_no_view() {
        assert!(matches!(P99MainBuilding.evaluate(&nbhd(vec![building_at("B1", 0.0, 0.0, 9.0)])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_uniform_field_has_no_dominant_tall_building() {
        let n = nbhd(vec![
            building_at("B1", -50.0, 0.0, 9.0), building_at("B2", 0.0, 0.0, 9.0), building_at("B3", 50.0, 0.0, 9.0),
        ]);
        let out = P99MainBuilding.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["height_dominance"] - 0.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_tall_central_building_scores_high_on_both() {
        let n = nbhd(vec![
            building_at("EDGE1", -50.0, 0.0, 9.0), building_at("EDGE2", 50.0, 0.0, 9.0),
            building_at("EDGE3", 0.0, 50.0, 9.0), building_at("TALL_CENTER", 0.0, 0.0, 30.0),
        ]);
        let out = P99MainBuilding.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, details, .. } => {
                assert!((sub_scores["height_dominance"] - 1.0).abs() < 1e-6);
                assert!(sub_scores["centrality"] > 0.6, "got {}", sub_scores["centrality"]);
                assert_eq!(details["tallest_building"], "TALL_CENTER");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_tall_but_off_center_building_scores_low_centrality() {
        let n = nbhd(vec![
            building_at("EDGE1", -20.0, 0.0, 9.0), building_at("EDGE2", 0.0, 0.0, 9.0),
            building_at("EDGE3", 20.0, 0.0, 9.0), building_at("TALL_FAR", 2000.0, 0.0, 30.0),
        ]);
        let out = P99MainBuilding.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!(sub_scores["centrality"] < 0.2, "got {}", sub_scores["centrality"]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
