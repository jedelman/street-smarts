//! P53 Main Gateways — every boundary with real human meaning should be
//! marked by a gateway where the paths that cross it actually cross.
//!
//! From Alexander, *A Pattern Language*, Pattern 53 (p. 276), via
//! patternlanguage.cc/Patterns/Main-Gateways-(53):
//! > **Problem:** Any part of town -- large or small -- which is to be
//! > identified by its inhabitants as a precinct of some kind, will be
//! > reinforced, helped in its distinctness, marked, and made more vivid,
//! > if the paths which enter it are marked by gateways where they cross
//! > the boundary.
//! > **Solution:** Mark every boundary in the city which has important
//! > human meaning -- the boundary of a building cluster, a neighborhood,
//! > a precinct -- by great gateways where the major entering paths cross
//! > the boundary.
//!
//! # A real, checkable proxy -- honestly `NoView` on real pipeline output
//! today
//!
//! `Neighborhood.boundaries: Vec<Boundary>` is a real, typed,
//! serializable field (`BoundaryKind::Natural`/`Infrastructural`/
//! `Jurisdictional`) -- but as of this writing no generator anywhere in
//! this pipeline populates it, the same class of real-field-no-producer
//! gap as `Parcel.use_category: "commercial"` (see `p33_night_life.rs`'s
//! own module doc) and `Neighborhood.activity_nodes`
//! (`p126_something_roughly_in_the_middle.rs`). This opinion checks, for
//! each real `Boundary`, whether a real street endpoint sits within
//! `GATEWAY_THRESHOLD_M` (30m) of the boundary's own centerline --
//! standing in for "a major entering path crosses the boundary here."
//! `value` = fraction of boundaries with at least one such crossing
//! street. Honestly `NoView` on every real pipeline output today, purely
//! because `boundaries` is always empty -- not a detector bug.

use std::collections::BTreeMap;
use street_smarts_core::geometry::point_to_polyline_m;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P53MainGateways;

const GATEWAY_THRESHOLD_M: f64 = 30.0;

impl Opinion for P53MainGateways {
    fn name(&self) -> &'static str {
        "p53_main_gateways"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p53".into(),
            display: "Alexander et al., A Pattern Language, Pattern 53 (Main Gateways)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Main-Gateways-(53)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        if n.boundaries.is_empty() {
            return OpinionOutput::NoView {
                reason: "No boundaries in this neighborhood -- no generator populates \
                         Neighborhood.boundaries yet. See this opinion's own module doc."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }
        if n.streets.is_empty() {
            return OpinionOutput::NoView {
                reason: "Boundaries present but no streets to check for a crossing gateway.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut gated = 0usize;
        let mut ungated: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for b in &n.boundaries {
            let nearest_m = n.streets.iter()
                .flat_map(|s| s.centerline.first().into_iter().chain(s.centerline.last()))
                .map(|p| point_to_polyline_m(p, &b.centerline))
                .fold(f64::MAX, f64::min);
            let has_gateway = nearest_m <= GATEWAY_THRESHOLD_M;
            details.insert(format!("{}.nearest_street_endpoint_m", b.id), format!("{:.1}", nearest_m.min(9999.0)));
            if has_gateway {
                gated += 1;
            } else {
                ungated.push(b.id.clone());
            }
        }

        let value = gated as f64 / n.boundaries.len() as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("gated_fraction".into(), value);
        details.insert("n_boundaries".into(), n.boundaries.len().to_string());
        details.insert("gateway_threshold_m".into(), format!("{GATEWAY_THRESHOLD_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} boundary/boundaries checked; {:.0}% have a real street endpoint within {:.0}m \
                 of the boundary's own centerline.",
                n.boundaries.len(), value * 100.0, GATEWAY_THRESHOLD_M
            ),
            sub_scores,
            details,
            caveats: vec![
                "A nearby street endpoint is a coarse stand-in for a real, built gateway structure \
                 -- no gateway/arch/marker concept exists anywhere in this schema.".into(),
                "Checks street ENDPOINTS only, not every real crossing of the boundary centerline by \
                 a street that passes through it.".into(),
            ],
            contributing_features: ungated,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::LngLat;
    use street_smarts_core::nir::{Boundary, BoundaryKind, NeighborhoodMeta, Street};

    fn nbhd(boundaries: Vec<Boundary>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings: vec![], streets, open_space: vec![],
            boundaries, activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P53 unit fixture".into(),
            },
        }
    }

    fn boundary(id: &str) -> Boundary {
        let m = 1.0 / 111_320.0;
        Boundary {
            id: id.into(),
            centerline: vec![LngLat::new(-100.0 * m, 0.0), LngLat::new(100.0 * m, 0.0)],
            kind: BoundaryKind::Jurisdictional,
        }
    }

    #[test]
    fn no_boundaries_is_no_view() {
        assert!(matches!(P53MainGateways.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn no_streets_is_no_view() {
        let n = nbhd(vec![boundary("B1")], vec![]);
        assert!(matches!(P53MainGateways.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_street_ending_at_the_boundary_scores_full() {
        let m = 1.0 / 111_320.0;
        let street = Street {
            id: "S1".into(),
            centerline: vec![LngLat::new(0.0, -50.0 * m), LngLat::new(0.0, 0.0)],
            classification: Some("local".into()), row_width_m: Some(5.5), surface: None,
        };
        let n = nbhd(vec![boundary("B1")], vec![street]);
        match P53MainGateways.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_distant_street_scores_zero() {
        let m = 1.0 / 111_320.0;
        let street = Street {
            id: "S1".into(),
            centerline: vec![LngLat::new(500.0 * m, 500.0 * m), LngLat::new(600.0 * m, 500.0 * m)],
            classification: Some("local".into()), row_width_m: Some(5.5), surface: None,
        };
        let n = nbhd(vec![boundary("B1")], vec![street]);
        match P53MainGateways.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-6, "got {value}");
                assert!(contributing_features.contains(&"B1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
