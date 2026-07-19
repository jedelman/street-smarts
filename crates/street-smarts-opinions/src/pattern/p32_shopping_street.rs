//! P32 Shopping Street — a shopping street should run off a busier road,
//! and its own frontage should carry real shops, not parking.
//!
//! From Alexander, *A Pattern Language*, Pattern 32, via
//! patternlanguage.cc/Patterns/Shopping-Street-(32):
//! > **Problem:** Shopping centers depend on access: they need locations
//! > near major traffic arteries. However, the shoppers themselves don't
//! > benefit from traffic: they need quiet, comfort, and convenience...
//! > **Solution:** Encourage local shopping centers to grow in the form of
//! > short pedestrian streets, at right angles to major roads and opening
//! > off these roads -- with parking behind the shops, so that the cars
//! > can pull directly off the road, and yet not harm the shopping street.
//!
//! # Two real, checkable proxies
//!
//! - `commercial_frontage_fraction`: fraction of `Pedestrian`-classified
//!   streets with a real commercial-use parcel within
//!   `adjacency_threshold_m` (default 20m).
//! - `parking_kept_behind_fraction`: fraction of those streets with no
//!   `OpenSpaceKind::Parking` within the same threshold -- if parking
//!   exists at all, it shouldn't front the shopping street itself.
//!
//! `value = 0.5 * commercial_frontage_fraction + 0.5 * parking_kept_behind_fraction`.
//!
//! Cannot check "at right angles to major roads" -- no operator in this
//! pipeline currently produces an `Arterial`-classified street to test the
//! angle against. And since no operator currently produces
//! `OpenSpaceKind::Parking` either, `parking_kept_behind_fraction` is
//! vacuously 1.0 whenever no parking exists yet -- not real evidence
//! parking was deliberately placed behind the shops.

use std::collections::BTreeMap;
use street_smarts_core::geometry::point_to_polyline_m;
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P32ShoppingStreet;

const ADJACENCY_THRESHOLD_M: f64 = 20.0;

impl Opinion for P32ShoppingStreet {
    fn name(&self) -> &'static str {
        "p32_shopping_street"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p32".into(),
            display: "Alexander et al., A Pattern Language, Pattern 32 (Shopping Street)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Shopping-Street-(32)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let ped_streets: Vec<_> = n.streets.iter().filter(|s| s.classification.as_deref() == Some("pedestrian") && s.centerline.len() >= 2).collect();
        if ped_streets.is_empty() {
            return OpinionOutput::NoView {
                reason: "No Pedestrian-classified streets in this neighborhood yet.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }
        let commercial_parcels: Vec<_> = n.parcels.iter().filter(|p| p.use_category.as_deref() == Some("commercial")).collect();
        if commercial_parcels.is_empty() {
            return OpinionOutput::NoView {
                reason: "No commercial-use parcels to check shopping-street frontage against yet.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut frontage: Vec<bool> = Vec::new();
        let mut parking_behind: Vec<bool> = Vec::new();
        let mut weak: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for s in &ped_streets {
            let has_commercial = commercial_parcels.iter().any(|p| point_to_polyline_m(&p.polygon.centroid(), &s.centerline) <= ADJACENCY_THRESHOLD_M);
            let has_fronting_parking = n.open_space.iter().any(|o| o.kind == OpenSpaceKind::Parking && point_to_polyline_m(&o.polygon.centroid(), &s.centerline) <= ADJACENCY_THRESHOLD_M);
            frontage.push(has_commercial);
            parking_behind.push(!has_fronting_parking);
            details.insert(format!("{}.has_commercial_frontage", s.id), has_commercial.to_string());
            details.insert(format!("{}.parking_kept_behind", s.id), (!has_fronting_parking).to_string());
            if !has_commercial || has_fronting_parking {
                weak.push(s.id.clone());
            }
        }

        let commercial_fraction = frontage.iter().filter(|b| **b).count() as f64 / frontage.len() as f64;
        let parking_fraction = parking_behind.iter().filter(|b| **b).count() as f64 / parking_behind.len() as f64;
        let value = 0.5 * commercial_fraction + 0.5 * parking_fraction;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("commercial_frontage_fraction".into(), commercial_fraction);
        sub_scores.insert("parking_kept_behind_fraction".into(), parking_fraction);
        details.insert("n_pedestrian_streets".into(), ped_streets.len().to_string());
        details.insert("adjacency_threshold_m".into(), format!("{ADJACENCY_THRESHOLD_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} Pedestrian street(s); {:.0}% have real commercial frontage within {:.0}m, \
                 {:.0}% keep parking off that frontage.",
                ped_streets.len(), commercial_fraction * 100.0, ADJACENCY_THRESHOLD_M, parking_fraction * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Cannot check 'at right angles to major roads' -- no Arterial-classified street \
                 exists anywhere in this pipeline's current output to test the angle against.".into(),
                "No operator in this pipeline currently produces OpenSpaceKind::Parking at all, so \
                 parking_kept_behind_fraction is vacuously 1.0 whenever no parking exists yet -- \
                 not evidence parking was deliberately placed behind the shops.".into(),
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
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{NeighborhoodMeta, Parcel, Street};

    fn nbhd(parcels: Vec<Parcel>, streets: Vec<Street>, open_space: Vec<street_smarts_core::nir::OpenSpace>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels, buildings: vec![], streets, open_space,
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P32 unit fixture".into(),
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

    fn ped_street(y_m: f64) -> Street {
        let m = 1.0 / 111_320.0;
        Street { id: "PED1".into(), centerline: vec![LngLat::new(-50.0 * m, y_m * m), LngLat::new(50.0 * m, y_m * m)], classification: Some("pedestrian".into()), row_width_m: Some(4.0), surface: None }
    }

    #[test]
    fn no_pedestrian_streets_is_no_view() {
        assert!(matches!(P32ShoppingStreet.evaluate(&nbhd(vec![], vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn commercial_frontage_with_no_fronting_parking_scores_full() {
        let n = nbhd(vec![commercial_parcel("C1", 0.0, -5.0)], vec![ped_street(0.0)], vec![]);
        let out = P32ShoppingStreet.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn fronting_parking_is_flagged() {
        let parking = street_smarts_core::nir::OpenSpace { id: "PK1".into(), polygon: square(0.0, 3.0, 10.0), kind: OpenSpaceKind::Parking };
        let n = nbhd(vec![commercial_parcel("C1", 0.0, -5.0)], vec![ped_street(0.0)], vec![parking]);
        let out = P32ShoppingStreet.evaluate(&n);
        match out {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!(value < 1.0, "got {value}");
                assert!(contributing_features.contains(&"PED1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
