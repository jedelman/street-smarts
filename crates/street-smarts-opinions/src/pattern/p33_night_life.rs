//! P33 Night Life — evening-active uses should be knit together into real
//! centers, not scattered, and distributed evenly across the community.
//!
//! From Alexander, *A Pattern Language*, Pattern 33 (p. 179), via
//! patternlanguage.cc/Patterns/Night-Life-(33):
//! > **Problem:** Most of the city's activities close down at night;
//! > those which stay open won't do much for the night life of the city
//! > unless they are together.
//! > **Solution:** Knit together shops, amusements, and services which
//! > are open at night, along with hotels, bars, and all-night diners to
//! > form centers of night life... distributed evenly throughout the
//! > community.
//!
//! # A real, checkable proxy -- and a real, honest gap
//!
//! This pipeline has no operating-hours data anywhere in its schema --
//! "open at night" can't be checked at all, ever, without inventing data
//! that isn't real. What CAN be checked, same convention as
//! `p32_shopping_street.rs`/`p46_market_of_many_shops.rs`: whether
//! `use_category: "commercial"` parcels are actually KNIT TOGETHER
//! (clustered near each other), Alexander's other real, geometric
//! prescription in this same pattern.
//!
//! `value` = fraction of commercial parcels with at least one other
//! commercial parcel within `cluster_threshold_m` (default 60m).
//! `NoView` if fewer than 2 commercial parcels exist -- clustering needs
//! at least a pair. No operator in this pipeline currently produces
//! `use_category: "commercial"` at all, so this is `NoView` on every
//! real pipeline output today, same as its siblings.

use std::collections::BTreeMap;
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P33NightLife;

const CLUSTER_THRESHOLD_M: f64 = 60.0;

impl Opinion for P33NightLife {
    fn name(&self) -> &'static str {
        "p33_night_life"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p33".into(),
            display: "Alexander et al., A Pattern Language, Pattern 33 (Night Life)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Night-Life-(33)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let commercial: Vec<_> = n.parcels.iter().filter(|p| p.use_category.as_deref() == Some("commercial")).collect();
        if commercial.len() < 2 {
            return OpinionOutput::NoView {
                reason: format!(
                    "{} commercial-use parcel(s) in this neighborhood -- need at least 2 to check clustering.",
                    commercial.len()
                ),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let centroids: Vec<_> = commercial.iter().map(|p| p.polygon.centroid()).collect();
        let n_clustered = centroids.iter().enumerate()
            .filter(|(i, c)| centroids.iter().enumerate().any(|(j, other)| *i != j && haversine_m(c, other) <= CLUSTER_THRESHOLD_M))
            .count();
        let value = n_clustered as f64 / commercial.len() as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("clustered_fraction".into(), value);

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_commercial_parcels".into(), commercial.len().to_string());
        details.insert("n_clustered".into(), n_clustered.to_string());
        details.insert("cluster_threshold_m".into(), format!("{CLUSTER_THRESHOLD_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} commercial parcel(s); {}/{} ({:.0}%) sit within {:.0}m of another commercial \
                 parcel, forming a real cluster.",
                commercial.len(), n_clustered, commercial.len(), value * 100.0, CLUSTER_THRESHOLD_M
            ),
            sub_scores,
            details,
            caveats: vec![
                "Cannot check 'open at night' at all -- no operating-hours data exists anywhere in \
                 this pipeline's schema, and inventing one wouldn't be real evidence. Checks the \
                 'knit together' clustering claim only.".into(),
                "No operator in this pipeline currently produces use_category: \"commercial\", so \
                 this is NoView on every real pipeline output today -- same real gap \
                 p32_shopping_street.rs and p46_market_of_many_shops.rs already document.".into(),
                "Doesn't check 'distributed evenly throughout the community' -- a single tight \
                 cluster of commercial parcels scores identically to several smaller ones spread \
                 across the site.".into(),
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
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{NeighborhoodMeta, Parcel};

    fn nbhd(parcels: Vec<Parcel>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels, buildings: vec![], streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P33 unit fixture".into(),
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
        Parcel { id: id.into(), polygon: square(x_m, y_m, 8.0), area_acres: 0.02, use_category: Some("commercial".into()), ownership: None, is_eda: false, spec: None, density_tier: None, target_stories: None }
    }

    #[test]
    fn fewer_than_two_commercial_parcels_is_no_view() {
        assert!(matches!(P33NightLife.evaluate(&nbhd(vec![commercial("C1", 0.0, 0.0)])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn two_nearby_commercial_parcels_form_a_cluster() {
        let n = nbhd(vec![commercial("C1", 0.0, 0.0), commercial("C2", 30.0, 0.0)]);
        let out = P33NightLife.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn two_distant_commercial_parcels_do_not_cluster() {
        let n = nbhd(vec![commercial("C1", 0.0, 0.0), commercial("C2", 500.0, 0.0)]);
        let out = P33NightLife.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 0.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
