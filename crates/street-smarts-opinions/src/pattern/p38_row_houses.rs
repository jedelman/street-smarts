//! P38 Row Houses — buildings along a pedestrian path should have a long
//! frontage and a shallow depth, not a square or deep footprint.
//!
//! From Alexander, *A Pattern Language*, Pattern 38, via
//! patternlanguage.cc/Patterns/Row-Houses-(38):
//! > **Problem:** At densities of 15 to 30 houses per acre, row houses
//! > are essential. But typical row houses are dark inside, and stamped
//! > from an identical mould.
//! > **Solution:** For row houses, place houses along pedestrian paths
//! > that run at right angles to local roads and parking lots, and give
//! > each house a long frontage and a shallow depth.
//!
//! # A real, checkable shape proxy
//!
//! For each building within `adjacency_threshold_m` (default 20m) of a
//! `Pedestrian`-classified street, this opinion computes the ratio of its
//! local bounding-box long dimension to its short dimension -- Alexander's
//! own "long frontage, shallow depth" as a real, checkable aspect ratio,
//! scored against `min_aspect_ratio` (default 1.5).
//!
//! `value` = fraction of pedestrian-adjacent buildings meeting the
//! aspect-ratio target. `NoView` if no building sits near a real
//! pedestrian street.

use std::collections::BTreeMap;
use street_smarts_core::geometry::{haversine_m, LngLat};
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P38RowHouses;

const ADJACENCY_THRESHOLD_M: f64 = 20.0;
const MIN_ASPECT_RATIO: f64 = 1.5;

fn bbox_dims_m(ring: &[LngLat]) -> (f64, f64) {
    if ring.is_empty() {
        return (0.0, 0.0);
    }
    let lat0 = ring.iter().map(|p| p.lat).sum::<f64>() / ring.len() as f64;
    let mlat = lat0.to_radians().cos();
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in ring {
        let x = p.lng * mlat * 111_320.0;
        let y = p.lat * 110_540.0;
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    let (dx, dy) = (max_x - min_x, max_y - min_y);
    (dx.max(dy), dx.min(dy))
}

impl Opinion for P38RowHouses {
    fn name(&self) -> &'static str {
        "p38_row_houses"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p38".into(),
            display: "Alexander et al., A Pattern Language, Pattern 38 (Row Houses)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Row-Houses-(38)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let pedestrian_points: Vec<LngLat> = n.streets.iter()
            .filter(|s| s.classification.as_deref() == Some("pedestrian"))
            .flat_map(|s| s.centerline.iter().copied())
            .collect();
        if pedestrian_points.is_empty() {
            return OpinionOutput::NoView {
                reason: "No Pedestrian-classified streets in this neighborhood.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut ratios: Vec<f64> = Vec::new();
        let mut shallow: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for b in &n.buildings {
            let bc = b.polygon.centroid();
            let near = pedestrian_points.iter().any(|p| haversine_m(&bc, p) <= ADJACENCY_THRESHOLD_M);
            if !near {
                continue;
            }
            let (long, short) = bbox_dims_m(&b.polygon.outer);
            if short <= 0.0 {
                continue;
            }
            let ratio = long / short;
            ratios.push(ratio);
            details.insert(format!("{}.aspect_ratio", b.id), format!("{ratio:.2}"));
            if ratio < MIN_ASPECT_RATIO {
                shallow.push(b.id.clone());
            }
        }

        if ratios.is_empty() {
            return OpinionOutput::NoView {
                reason: "No building sits near a real Pedestrian-classified street.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let value = ratios.iter().filter(|r| **r >= MIN_ASPECT_RATIO).count() as f64 / ratios.len() as f64;
        let mean_ratio = ratios.iter().sum::<f64>() / ratios.len() as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("long_frontage_fraction".into(), value);
        sub_scores.insert("mean_aspect_ratio".into(), mean_ratio);

        details.insert("n_pedestrian_adjacent_buildings".into(), ratios.len().to_string());
        details.insert("min_aspect_ratio_threshold".into(), format!("{MIN_ASPECT_RATIO:.1}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} building(s) near a Pedestrian street; mean aspect ratio {:.2}; {:.0}% meet the \
                 {:.1}x long-frontage target.",
                ratios.len(), mean_ratio, value * 100.0, MIN_ASPECT_RATIO
            ),
            sub_scores,
            details,
            caveats: vec![
                "Aspect ratio (bounding-box long/short) doesn't confirm the LONG side actually \
                 faces the pedestrian path -- a building could be elongated in the wrong \
                 direction and still pass.".into(),
                "Doesn't check 'at right angles to local roads and parking lots' at all -- no \
                 check of the pedestrian path's own relationship to nearby local roads.".into(),
                "Doesn't check the 15-30 houses/acre density range this pattern is scoped to -- \
                 applies the same aspect-ratio check regardless of local density.".into(),
            ],
            contributing_features: shallow,
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
                layer_provenance: Default::default(), label: "P38 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn ped_street() -> Street {
        let m = 1.0 / 111_320.0;
        Street { id: "PED1".into(), centerline: vec![LngLat::new(0.0, -5.0 * m), LngLat::new(200.0 * m, -5.0 * m)], classification: Some("pedestrian".into()), row_width_m: Some(4.0), surface: None }
    }

    fn rect_building(id: &str, x_m: f64, w_m: f64, h_m: f64) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(x_m * m, 0.0), LngLat::new((x_m + w_m) * m, 0.0),
                LngLat::new((x_m + w_m) * m, h_m * m), LngLat::new(x_m * m, h_m * m), LngLat::new(x_m * m, 0.0),
            ]),
            height_m: Some(6.0), typology: Some("p107_solid_v01".into()), year_built: None, parcel_id: None, floors: Some(2), openings: vec![], interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], }
    }

    #[test]
    fn no_pedestrian_streets_is_no_view() {
        assert!(matches!(P38RowHouses.evaluate(&nbhd(vec![rect_building("B1", 0.0, 10.0, 10.0)], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_long_shallow_building_meets_the_target() {
        let n = nbhd(vec![rect_building("B1", 0.0, 15.0, 6.0)], vec![ped_street()]);
        let out = P38RowHouses.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_square_building_fails_the_target() {
        let n = nbhd(vec![rect_building("B1", 0.0, 10.0, 10.0)], vec![ped_street()]);
        let out = P38RowHouses.evaluate(&n);
        match out {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-6);
                assert!(contributing_features.contains(&"B1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
