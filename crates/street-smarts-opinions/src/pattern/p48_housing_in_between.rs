//! P48 Housing In Between — housing should be woven directly into
//! nonresidential fabric, not sharply zoned apart from it.
//!
//! From Alexander, *A Pattern Language*, Pattern 48, via
//! patternlanguage.cc/Patterns/Housing-In-Between-(48):
//! > **Problem:** Wherever there is a sharp separation between
//! > residential and nonresidential parts of town, the nonresidential
//! > areas will quickly turn to slums.
//! > **Solution:** Build houses into the fabric of shops, small industry,
//! > schools, public services, universities -- all those parts of cities
//! > which draw people in during the day, but which tend to be
//! > "nonresidential."
//!
//! # A real, checkable proxy
//!
//! For each residential-use parcel, this opinion checks whether a
//! nonresidential parcel (any `use_category` other than "residential",
//! "vacant", or unset) sits within `adjacency_threshold_m` (default 25m).
//! `value` = fraction of residential parcels with real nonresidential
//! adjacency -- housing genuinely woven into the fabric, not a
//! residential-only zone with no daytime draw nearby.

use std::collections::BTreeMap;
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P48HousingInBetween;

const ADJACENCY_THRESHOLD_M: f64 = 25.0;

fn is_nonresidential(use_category: &Option<String>) -> bool {
    match use_category.as_deref() {
        None | Some("residential") | Some("vacant") => false,
        Some(_) => true,
    }
}

impl Opinion for P48HousingInBetween {
    fn name(&self) -> &'static str {
        "p48_housing_in_between"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p48".into(),
            display: "Alexander et al., A Pattern Language, Pattern 48 (Housing In Between)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Housing-In-Between-(48)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let residential: Vec<_> = n.parcels.iter().filter(|p| p.use_category.as_deref() == Some("residential")).collect();
        if residential.is_empty() {
            return OpinionOutput::NoView {
                reason: "No residential-use parcels in this neighborhood yet.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }
        let nonresidential: Vec<_> = n.parcels.iter().filter(|p| is_nonresidential(&p.use_category)).collect();
        if nonresidential.is_empty() {
            return OpinionOutput::NoView {
                reason: "No nonresidential parcels to check housing adjacency against yet.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut woven_in: Vec<bool> = Vec::new();
        let mut isolated: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for r in &residential {
            let rc = r.polygon.centroid();
            let nearest = nonresidential.iter().map(|p| haversine_m(&rc, &p.polygon.centroid())).fold(f64::MAX, f64::min);
            let ok = nearest <= ADJACENCY_THRESHOLD_M;
            woven_in.push(ok);
            details.insert(format!("{}.nearest_nonresidential_m", r.id), format!("{:.1}", nearest.min(9999.0)));
            if !ok {
                isolated.push(r.id.clone());
            }
        }

        let value = woven_in.iter().filter(|b| **b).count() as f64 / woven_in.len() as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("woven_into_nonresidential_fabric".into(), value);
        details.insert("n_residential_parcels".into(), residential.len().to_string());
        details.insert("adjacency_threshold_m".into(), format!("{ADJACENCY_THRESHOLD_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} residential parcel(s); {:.0}% sit within {:.0}m of a real nonresidential \
                 (shop/industry/school/civic) parcel.",
                residential.len(), value * 100.0, ADJACENCY_THRESHOLD_M
            ),
            sub_scores,
            details,
            caveats: vec![
                "'Nonresidential' is any Parcel.use_category other than 'residential', 'vacant', \
                 or unset -- a free-form string this pipeline doesn't otherwise constrain, so the \
                 real range of nonresidential categories in a given fixture may be narrower than \
                 Alexander's own list (shops, industry, schools, universities).".into(),
                "Checks proximity only -- doesn't confirm the housing is genuinely built INTO the \
                 fabric (e.g. above or beside a specific shop) versus merely being the nearest \
                 residential parcel to it.".into(),
            ],
            contributing_features: isolated,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{NeighborhoodMeta, Parcel};

    fn nbhd(parcels: Vec<Parcel>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels, buildings: vec![], streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P48 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn parcel_at(id: &str, x_m: f64, y_m: f64, use_category: Option<&str>) -> Parcel {
        let m = 1.0 / 111_320.0;
        let side = 8.0;
        Parcel {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(x_m * m, y_m * m), LngLat::new((x_m + side) * m, y_m * m),
                LngLat::new((x_m + side) * m, (y_m + side) * m), LngLat::new(x_m * m, (y_m + side) * m),
                LngLat::new(x_m * m, y_m * m),
            ]),
            area_acres: 0.02, use_category: use_category.map(String::from), ownership: None, is_eda: false, spec: None, density_tier: None, target_stories: None,
        }
    }

    #[test]
    fn no_residential_parcels_is_no_view() {
        assert!(matches!(P48HousingInBetween.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn residential_next_to_shops_scores_full() {
        let n = nbhd(vec![parcel_at("R1", 0.0, 0.0, Some("residential")), parcel_at("C1", 10.0, 0.0, Some("commercial"))]);
        let out = P48HousingInBetween.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn isolated_residential_enclave_is_flagged() {
        let n = nbhd(vec![parcel_at("R1", 0.0, 0.0, Some("residential")), parcel_at("C1", 5000.0, 5000.0, Some("commercial"))]);
        let out = P48HousingInBetween.evaluate(&n);
        match out {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-6);
                assert!(contributing_features.contains(&"R1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
