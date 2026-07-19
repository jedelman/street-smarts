//! P88 Street Cafe — cafes should sit right on busy streets, spilling
//! out onto them, not set back or hidden.
//!
//! From Alexander, *A Pattern Language*, Pattern 88 (p. 436), via
//! patternlanguage.cc/Patterns/Street-Cafe-(88):
//! > **Problem:** The street cafe provides a unique setting, special to
//! > cities: a place where people can sit lazily, legitimately, be on
//! > view, and watch the world go by.
//! > **Solution:** Promote neighborhood cafes designed as intimate
//! > multi-room spaces opening onto busy streets... tables extending
//! > from indoors into the street.
//!
//! # A real, checkable proxy
//!
//! Same convention as `p32_shopping_street.rs`: a real, checkable proxy
//! for "opening onto a busy street" is real adjacency -- a commercial
//! parcel within `adjacency_threshold_m` (default 15m, tighter than
//! P32's 20m since a cafe needs direct street frontage, not just
//! nearby access) of a Local OR Pedestrian classified street (either
//! reads as "busy" at this pipeline's scale; no Arterial classification
//! exists to distinguish further).
//!
//! `value` = fraction of commercial parcels with real street frontage
//! within that threshold. Cannot check "intimate multi-room" (no
//! interior partition data on non-Building parcels) or "tables extending
//! into the street" (no furniture/program-detail data at all).

use std::collections::BTreeMap;
use street_smarts_core::geometry::point_to_polyline_m;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P88StreetCafe;

const ADJACENCY_THRESHOLD_M: f64 = 15.0;

impl Opinion for P88StreetCafe {
    fn name(&self) -> &'static str {
        "p88_street_cafe"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p88".into(),
            display: "Alexander et al., A Pattern Language, Pattern 88 (Street Cafe)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Street-Cafe-(88)".into()),
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
                reason: "No commercial-use parcels to check street-cafe frontage against yet.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }
        let streets: Vec<_> = n.streets.iter()
            .filter(|s| matches!(s.classification.as_deref(), Some("local") | Some("pedestrian")) && s.centerline.len() >= 2)
            .collect();
        if streets.is_empty() {
            return OpinionOutput::NoView {
                reason: "No Local or Pedestrian streets in this neighborhood yet -- run PathNetwork first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut fronting: Vec<bool> = Vec::new();
        let mut weak: Vec<String> = Vec::new();
        for p in &commercial {
            let centroid = p.polygon.centroid();
            let has_frontage = streets.iter().any(|s| point_to_polyline_m(&centroid, &s.centerline) <= ADJACENCY_THRESHOLD_M);
            fronting.push(has_frontage);
            if !has_frontage {
                weak.push(p.id.clone());
            }
        }
        let value = fronting.iter().filter(|b| **b).count() as f64 / fronting.len() as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("street_frontage_fraction".into(), value);

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_commercial_parcels".into(), commercial.len().to_string());
        details.insert("adjacency_threshold_m".into(), format!("{ADJACENCY_THRESHOLD_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} commercial parcel(s); {:.0}% sit within {:.0}m of a Local or Pedestrian street.",
                commercial.len(), value * 100.0, ADJACENCY_THRESHOLD_M
            ),
            sub_scores,
            details,
            caveats: vec![
                "No operator in this pipeline currently produces use_category: \"commercial\", so \
                 this is NoView on every real pipeline output today -- same real gap \
                 p32_shopping_street.rs already documents.".into(),
                "Cannot check 'intimate multi-room' interior layout (no interior partition data \
                 exists on Parcel, only on Building) or 'tables extending into the street' (no \
                 furniture/program-detail data at all).".into(),
                "Adjacency, not confirmed frontage -- a parcel could be within threshold of a \
                 street on its BACK side, not actually facing it.".into(),
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

    fn nbhd(parcels: Vec<Parcel>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels, buildings: vec![], streets, open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P88 unit fixture".into(),
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

    fn local_street(y_m: f64) -> Street {
        let m = 1.0 / 111_320.0;
        Street { id: "S1".into(), centerline: vec![LngLat::new(-50.0 * m, y_m * m), LngLat::new(50.0 * m, y_m * m)], classification: Some("local".into()), row_width_m: Some(5.5), surface: None }
    }

    #[test]
    fn no_commercial_parcels_is_no_view() {
        assert!(matches!(P88StreetCafe.evaluate(&nbhd(vec![], vec![local_street(0.0)])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn no_streets_is_no_view() {
        assert!(matches!(P88StreetCafe.evaluate(&nbhd(vec![commercial("C1", 0.0, -5.0)], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_cafe_right_on_the_street_scores_full() {
        let n = nbhd(vec![commercial("C1", 0.0, -5.0)], vec![local_street(0.0)]);
        let out = P88StreetCafe.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_set_back_cafe_scores_zero() {
        let n = nbhd(vec![commercial("C1", 0.0, -100.0)], vec![local_street(0.0)]);
        let out = P88StreetCafe.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 0.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
