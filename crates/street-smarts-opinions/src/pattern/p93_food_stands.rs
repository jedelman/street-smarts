//! P93 Food Stands — food vendors should concentrate at real
//! intersections, where cars and pedestrian paths meet.
//!
//! From Alexander, *A Pattern Language*, Pattern 93 (p. 454), via
//! patternlanguage.cc/Patterns/Food-Stands-(93):
//! > **Problem:** Many of our habits and institutions are bolstered by
//! > the fact that we can get simple, inexpensive food on the street, on
//! > the way to shopping, work, and friends.
//! > **Solution:** Concentrate food stands where cars and paths meet --
//! > either portable stands or small huts, or built into the fronts of
//! > buildings, half-open to the street.
//!
//! # A real, checkable proxy
//!
//! Real intersections are found the same way `p49_looped_local_roads.rs`
//! and `p50_t_junctions.rs` find them: streets sharing a
//! coordinate-snapped endpoint. For each real intersection (2+ streets
//! meeting), this checks whether a real `use_category: "commercial"`
//! parcel sits within `adjacency_threshold_m` (default 20m) of it --
//! "where cars and paths meet," not just anywhere along a street.
//!
//! `value` = fraction of real intersections with a commercial parcel
//! nearby.

use std::collections::BTreeMap;
use street_smarts_core::geometry::{haversine_m, LngLat};
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P93FoodStands;

const ADJACENCY_THRESHOLD_M: f64 = 20.0;

fn node_key(p: &LngLat) -> (i64, i64) {
    ((p.lng * 1e7).round() as i64, (p.lat * 1e7).round() as i64)
}

impl Opinion for P93FoodStands {
    fn name(&self) -> &'static str {
        "p93_food_stands"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p93".into(),
            display: "Alexander et al., A Pattern Language, Pattern 93 (Food Stands)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Food-Stands-(93)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let mut node_points: std::collections::HashMap<(i64, i64), LngLat> = std::collections::HashMap::new();
        let mut node_count: std::collections::HashMap<(i64, i64), usize> = std::collections::HashMap::new();
        for s in &n.streets {
            if s.centerline.len() < 2 {
                continue;
            }
            for endpoint in [s.centerline.first().unwrap(), s.centerline.last().unwrap()] {
                let k = node_key(endpoint);
                node_points.entry(k).or_insert(*endpoint);
                *node_count.entry(k).or_insert(0) += 1;
            }
        }
        let intersections: Vec<&LngLat> = node_count.iter()
            .filter(|(_, &c)| c >= 2)
            .filter_map(|(k, _)| node_points.get(k))
            .collect();
        if intersections.is_empty() {
            return OpinionOutput::NoView {
                reason: "No real street intersections in this neighborhood -- run PathNetwork first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }
        let commercial: Vec<_> = n.parcels.iter().filter(|p| p.use_category.as_deref() == Some("commercial")).collect();
        if commercial.is_empty() {
            return OpinionOutput::NoView {
                reason: "No commercial-use parcels to check food-stand concentration against yet.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let n_with_stand = intersections.iter()
            .filter(|node| commercial.iter().any(|p| haversine_m(&p.polygon.centroid(), node) <= ADJACENCY_THRESHOLD_M))
            .count();
        let value = n_with_stand as f64 / intersections.len() as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("intersections_with_commercial_nearby".into(), value);

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_intersections".into(), intersections.len().to_string());
        details.insert("n_with_commercial_nearby".into(), n_with_stand.to_string());
        details.insert("adjacency_threshold_m".into(), format!("{ADJACENCY_THRESHOLD_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} real intersection(s); {}/{} ({:.0}%) have a commercial parcel within {:.0}m.",
                intersections.len(), n_with_stand, intersections.len(), value * 100.0, ADJACENCY_THRESHOLD_M
            ),
            sub_scores,
            details,
            caveats: vec![
                "No operator in this pipeline currently produces use_category: \"commercial\", so \
                 this is NoView on every real pipeline output today -- same real gap \
                 p32_shopping_street.rs already documents.".into(),
                "Cannot distinguish an actual food stand from any other commercial use -- checks \
                 real commercial-parcel presence near an intersection, not the specific vendor \
                 type Alexander describes.".into(),
                "Node identity is coordinate-snapping (1cm precision) on real street endpoints, \
                 same convention as p49/p50/p52 -- a geometric crossing without a shared endpoint \
                 vertex isn't counted as an intersection.".into(),
            ],
            contributing_features: vec![],
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{NeighborhoodMeta, Parcel, Street};

    fn nbhd(parcels: Vec<Parcel>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels, buildings: vec![], streets, open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P93 unit fixture".into(),
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

    fn commercial(id: &str, x_m: f64, y_m: f64) -> Parcel {
        Parcel { id: id.into(), polygon: square(x_m, y_m, 6.0), area_acres: 0.01, use_category: Some("commercial".into()), ownership: None, is_eda: false, spec: None, density_tier: None, target_stories: None }
    }

    fn street(id: &str, a: (f64, f64), b: (f64, f64)) -> Street {
        let m = 1.0 / 111_320.0;
        Street { id: id.into(), centerline: vec![LngLat::new(a.0 * m, a.1 * m), LngLat::new(b.0 * m, b.1 * m)], classification: Some("local".into()), row_width_m: Some(5.5) }
    }

    #[test]
    fn no_intersections_is_no_view() {
        let n = nbhd(vec![], vec![street("S1", (0.0, 0.0), (100.0, 0.0))]);
        assert!(matches!(P93FoodStands.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn no_commercial_parcels_is_no_view() {
        let n = nbhd(vec![], vec![
            street("S1", (-100.0, 0.0), (0.0, 0.0)),
            street("S2", (0.0, 0.0), (0.0, 100.0)),
        ]);
        assert!(matches!(P93FoodStands.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_food_stand_at_the_intersection_scores_full() {
        let n = nbhd(vec![commercial("C1", 5.0, 5.0)], vec![
            street("S1", (-100.0, 0.0), (0.0, 0.0)),
            street("S2", (0.0, 0.0), (0.0, 100.0)),
        ]);
        let out = P93FoodStands.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_commercial_parcel_far_from_any_intersection_scores_zero() {
        let n = nbhd(vec![commercial("C1", 500.0, 500.0)], vec![
            street("S1", (-100.0, 0.0), (0.0, 0.0)),
            street("S2", (0.0, 0.0), (0.0, 100.0)),
        ]);
        let out = P93FoodStands.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 0.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
