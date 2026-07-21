//! P89 Corner Grocery — a neighborhood needs a small grocery at a real
//! corner, combined with housing so its keeper can live there.
//!
//! From Alexander, *A Pattern Language*, Pattern 89, via
//! patternlanguage.cc/Patterns/Corner-Grocery-(89):
//! > **Problem:** It has lately been assumed that people no longer want
//! > to walk to local stores. This assumption is mistaken.
//! > **Solution:** Give every neighborhood at least one corner grocery,
//! > somewhere near its heart... Place them on corners, where large
//! > numbers of people are going past. And combine them with houses, so
//! > that people who run them can live over them or next to them.
//!
//! # Two real, checkable proxies
//!
//! - `at_a_real_corner_fraction`: fraction of commercial-use parcels
//!   whose centroid sits within `corner_threshold_m` (default 15m) of
//!   TWO OR MORE distinct streets -- a real corner, not just fronting one
//!   road.
//! - `combined_with_housing_fraction`: fraction of commercial parcels
//!   whose building has 2 or more floors -- the real, checkable proxy for
//!   "people who run them can live over them" (no data exists to check
//!   whether the upper floor is actually residential, only that vertical
//!   space for it exists).
//!
//! `value = 0.5 * at_a_real_corner_fraction + 0.5 * combined_with_housing_fraction`.
//!
//! Cannot check the 200-800-yard spacing target or "serves about 100
//! people" -- no population or multi-grocery-instance spacing data exists
//! at this fixture's scale.

use std::collections::BTreeMap;
use street_smarts_core::geometry::point_to_polyline_m;
use street_smarts_core::nir::Neighborhood;
#[cfg(test)]
use street_smarts_core::geometry::LngLat;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P89CornerGrocery;

const CORNER_THRESHOLD_M: f64 = 15.0;

impl Opinion for P89CornerGrocery {
    fn name(&self) -> &'static str {
        "p89_corner_grocery"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p89".into(),
            display: "Alexander et al., A Pattern Language, Pattern 89 (Corner Grocery)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Corner-Grocery-(89)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let commercial: Vec<_> = n.parcels.iter().filter(|p| p.use_category.as_deref() == Some("commercial")).collect();
        if commercial.is_empty() {
            return OpinionOutput::NoView {
                reason: "No commercial-use parcels in this neighborhood yet.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }
        if n.streets.is_empty() {
            return OpinionOutput::NoView {
                reason: "No streets to check corner adjacency against yet.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut at_corner: Vec<bool> = Vec::new();
        let mut combined_with_housing: Vec<bool> = Vec::new();
        let mut weak: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for p in &commercial {
            let c = p.polygon.centroid();
            let n_nearby_streets = n.streets.iter().filter(|s| point_to_polyline_m(&c, &s.centerline) <= CORNER_THRESHOLD_M).count();
            let corner = n_nearby_streets >= 2;
            at_corner.push(corner);

            let has_upper_floor = n.buildings.iter()
                .filter(|b| b.parcel_id.as_deref() == Some(p.id.as_str()))
                .any(|b| b.floors.unwrap_or(1) >= 2);
            combined_with_housing.push(has_upper_floor);

            details.insert(format!("{}.n_nearby_streets", p.id), n_nearby_streets.to_string());
            details.insert(format!("{}.has_upper_floor", p.id), has_upper_floor.to_string());
            if !corner || !has_upper_floor {
                weak.push(p.id.clone());
            }
        }

        let corner_fraction = at_corner.iter().filter(|b| **b).count() as f64 / at_corner.len() as f64;
        let housing_fraction = combined_with_housing.iter().filter(|b| **b).count() as f64 / combined_with_housing.len() as f64;
        let value = 0.5 * corner_fraction + 0.5 * housing_fraction;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("at_a_real_corner_fraction".into(), corner_fraction);
        sub_scores.insert("combined_with_housing_fraction".into(), housing_fraction);
        details.insert("n_commercial_parcels".into(), commercial.len().to_string());
        details.insert("corner_threshold_m".into(), format!("{CORNER_THRESHOLD_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} commercial parcel(s); {:.0}% sit at a real corner (>=2 streets within {:.0}m), \
                 {:.0}% have a real second floor to live over the shop.",
                commercial.len(), corner_fraction * 100.0, CORNER_THRESHOLD_M, housing_fraction * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Cannot check the 200-800-yard spacing target ('according to the density') or \
                 'serves about 100 people' -- no population data or confirmed multi-grocery \
                 instance spacing exists at this fixture's scale.".into(),
                "A second floor only shows vertical space exists to live over the shop, not that \
                 it's actually occupied as housing -- Building.typology/use has no residential-\
                 above-commercial designation in this schema.".into(),
            ],
            contributing_features: weak,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{Building, NeighborhoodMeta, Parcel, Street};

    fn nbhd(parcels: Vec<Parcel>, buildings: Vec<Building>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels, buildings, streets, open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P89 unit fixture".into(),
            },
        }
    }

    fn square(x_m: f64, y_m: f64, side: f64) -> Polygon {
        let m = 1.0 / 111_320.0;
        Polygon::from_ring(vec![
            LngLat::new(x_m * m, y_m * m), LngLat::new((x_m + side) * m, y_m * m),
            LngLat::new((x_m + side) * m, (y_m + side) * m), LngLat::new(x_m * m, (y_m + side) * m),
            LngLat::new(x_m * m, y_m * m),
        ])
    }

    fn commercial_parcel(id: &str, x_m: f64, y_m: f64) -> Parcel {
        Parcel { id: id.into(), polygon: square(x_m, y_m, 8.0), area_acres: 0.02, use_category: Some("commercial".into()), ownership: None, is_eda: false, spec: None, density_tier: None, target_stories: None }
    }

    fn building_on(id: &str, parcel_id: &str, x_m: f64, y_m: f64, floors: u32) -> Building {
        Building { id: id.into(), polygon: square(x_m, y_m, 8.0), height_m: Some(6.0), typology: Some("p107_solid_v01".into()), year_built: None, parcel_id: Some(parcel_id.into()), floors: Some(floors), openings: vec![], interior_cells: vec![], wall_thickness_m: None, roof: None }
    }

    fn street_along_x(y_m: f64) -> Street {
        let m = 1.0 / 111_320.0;
        Street { id: format!("SX{y_m}"), centerline: vec![LngLat::new(-50.0 * m, y_m * m), LngLat::new(50.0 * m, y_m * m)], classification: Some("local".into()), row_width_m: Some(6.0), surface: None }
    }

    fn street_along_y(x_m: f64) -> Street {
        let m = 1.0 / 111_320.0;
        Street { id: format!("SY{x_m}"), centerline: vec![LngLat::new(x_m * m, -50.0 * m), LngLat::new(x_m * m, 50.0 * m)], classification: Some("local".into()), row_width_m: Some(6.0), surface: None }
    }

    #[test]
    fn no_commercial_parcels_is_no_view() {
        assert!(matches!(P89CornerGrocery.evaluate(&nbhd(vec![], vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_two_story_shop_at_an_intersection_scores_full() {
        let n = nbhd(
            vec![commercial_parcel("C1", 0.0, 0.0)],
            vec![building_on("B1", "C1", 0.0, 0.0, 2)],
            vec![street_along_x(-2.0), street_along_y(-2.0)],
        );
        let out = P89CornerGrocery.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_single_story_shop_on_a_single_street_is_flagged() {
        let n = nbhd(
            vec![commercial_parcel("C1", 0.0, 0.0)],
            vec![building_on("B1", "C1", 0.0, 0.0, 1)],
            vec![street_along_x(-2.0)],
        );
        let out = P89CornerGrocery.evaluate(&n);
        match out {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-6, "got {value}");
                assert!(contributing_features.contains(&"C1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
