//! P30 Activity Nodes — small public squares should sit where paths
//! actually converge, spaced roughly 300 yards apart, not scattered
//! independent of the path network.
//!
//! From Alexander, *A Pattern Language*, Pattern 30, via
//! patternlanguage.cc/Patterns/Activity-Nodes-(30):
//! > **Problem:** Community facilities scattered individually through the
//! > city do nothing for the life of the city.
//! > **Solution:** Create nodes of activity throughout the community,
//! > spread about 300 yards apart. First identify those existing spots in
//! > the community where action seems to concentrate itself. Then modify
//! > the layout of the paths in the community to bring as many of them
//! > through these spots as possible.
//!
//! # A schema gap worth naming up front, not hidden
//!
//! `Neighborhood.activity_nodes: Vec<ActivityNode>` exists in the NIR
//! schema (see `nir.rs`) but no current pattern operator ever populates
//! it -- confirmed by grep across `street-smarts-patterns/src`, not
//! assumed. This opinion does NOT read that field (it would be
//! permanently empty and this opinion would be permanently NoView, which
//! isn't useful). It checks the same underlying claim -- "activity
//! concentrates where paths converge" -- against real, populated data
//! instead: `p61_small_public_squares`'s `Plaza`-kind open space and
//! `path_network`'s real `Street` topology.
//!
//! # Two sub-scores
//!
//! - `path_convergence`: fraction of real `Plaza` open-space features that
//!   have at least two distinct streets with an endpoint within
//!   `convergence_threshold_m` (default 30m) of the plaza's centroid --
//!   a real check that squares sit where paths actually meet, not
//!   independent of the path network.
//! - `spacing`: how close the mean nearest-neighbor distance between
//!   qualifying plazas is to Alexander's literal "~300 yards" (274m)
//!   figure, scored `1.0` at exactly 274m falling off linearly to 0 at
//!   double or half that.
//!
//! `value = 0.5 * path_convergence + 0.5 * spacing`. `spacing` needs 2+
//! qualifying plazas to mean anything; with fewer, `value` = `path_convergence` alone.

use std::collections::BTreeMap;
use street_smarts_core::geometry::{haversine_m, LngLat};
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P30ActivityNodes;

const CONVERGENCE_THRESHOLD_M: f64 = 30.0;
const TARGET_SPACING_M: f64 = 274.0; // Alexander's literal "300 yards"

impl Opinion for P30ActivityNodes {
    fn name(&self) -> &'static str {
        "p30_activity_nodes"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p30".into(),
            display: "Alexander et al., A Pattern Language, Pattern 30 (Activity Nodes)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Activity-Nodes-(30)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let plazas: Vec<LngLat> = n.open_space.iter()
            .filter(|o| o.kind == OpenSpaceKind::Plaza && o.polygon.area_m2() > 0.0)
            .map(|o| o.polygon.centroid())
            .collect();
        if plazas.is_empty() {
            return OpinionOutput::NoView {
                reason: "No Plaza-kind open space in this neighborhood -- run P61 Small Public \
                         Squares first."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let street_endpoints: Vec<LngLat> = n.streets.iter()
            .filter_map(|s| s.centerline.first().copied())
            .chain(n.streets.iter().filter_map(|s| s.centerline.last().copied()))
            .collect();

        let mut converging: Vec<bool> = Vec::new();
        let mut not_converging: Vec<String> = Vec::new();
        for (i, plaza) in plazas.iter().enumerate() {
            let nearby_count = street_endpoints.iter()
                .filter(|e| haversine_m(plaza, e) <= CONVERGENCE_THRESHOLD_M)
                .count();
            let ok = nearby_count >= 2;
            converging.push(ok);
            if !ok {
                not_converging.push(format!("plaza_{i}"));
            }
        }
        let path_convergence = converging.iter().filter(|c| **c).count() as f64 / converging.len() as f64;

        let spacing = if plazas.len() < 2 {
            None
        } else {
            let mut nn_distances: Vec<f64> = Vec::new();
            for (i, a) in plazas.iter().enumerate() {
                let nearest = plazas.iter().enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, b)| haversine_m(a, b))
                    .fold(f64::MAX, f64::min);
                if nearest.is_finite() {
                    nn_distances.push(nearest);
                }
            }
            if nn_distances.is_empty() {
                None
            } else {
                let mean_nn = nn_distances.iter().sum::<f64>() / nn_distances.len() as f64;
                let deviation = (mean_nn - TARGET_SPACING_M).abs() / TARGET_SPACING_M;
                Some((1.0 - deviation).clamp(0.0, 1.0))
            }
        };

        let value = match spacing {
            Some(s) => 0.5 * path_convergence + 0.5 * s,
            None => path_convergence,
        };

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("path_convergence".into(), path_convergence);
        if let Some(s) = spacing {
            sub_scores.insert("spacing".into(), s);
        }

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_plazas".into(), plazas.len().to_string());
        details.insert("n_activity_nodes_field_populated".into(), n.activity_nodes.len().to_string());
        details.insert("target_spacing_m".into(), format!("{TARGET_SPACING_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} plaza(s); {:.0}% sit where 2+ real streets converge (within {:.0}m).{}",
                plazas.len(), path_convergence * 100.0, CONVERGENCE_THRESHOLD_M,
                spacing.map(|s| format!(" Spacing score {s:.2} against Alexander's ~300yd/274m target.")).unwrap_or_default(),
            ),
            sub_scores,
            details,
            caveats: vec![
                "Reads real Plaza open-space + Street topology, NOT Neighborhood.activity_nodes \
                 -- that NIR field exists but no current operator populates it (see this module's \
                 own doc comment).".into(),
                "path_convergence checks street ENDPOINTS near a plaza, not real mid-segment \
                 crossings or through-traffic volume -- a plaza near where two dead-end streets \
                 happen to stop scores the same as one at a genuine crossroads.".into(),
                "Doesn't check for real community facilities/shops surrounding each node -- no \
                 use/program data exists in this schema to verify Alexander's 'mutually supportive' \
                 shops requirement.".into(),
            ],
            contributing_features: not_converging,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{NeighborhoodMeta, OpenSpace, Street};

    fn nbhd(open_space: Vec<OpenSpace>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings: vec![], streets, open_space,
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P30 unit fixture".into(),
            },
        }
    }

    fn plaza_at(id: &str, x_m: f64, y_m: f64) -> OpenSpace {
        let m = 1.0 / 111_320.0;
        let half = 10.0;
        OpenSpace {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new((x_m - half) * m, (y_m - half) * m), LngLat::new((x_m + half) * m, (y_m - half) * m),
                LngLat::new((x_m + half) * m, (y_m + half) * m), LngLat::new((x_m - half) * m, (y_m + half) * m),
                LngLat::new((x_m - half) * m, (y_m - half) * m),
            ]),
            kind: OpenSpaceKind::Plaza,
        }
    }

    fn street_endpoint_at(id: &str, x_m: f64, y_m: f64, x2_m: f64, y2_m: f64) -> Street {
        let m = 1.0 / 111_320.0;
        Street { id: id.into(), centerline: vec![LngLat::new(x_m * m, y_m * m), LngLat::new(x2_m * m, y2_m * m)], classification: Some("local".into()), row_width_m: Some(6.0), surface: None }
    }

    #[test]
    fn no_plazas_is_no_view() {
        assert!(matches!(P30ActivityNodes.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn plaza_at_a_real_intersection_scores_full_convergence() {
        let n = nbhd(
            vec![plaza_at("P1", 0.0, 0.0)],
            vec![
                street_endpoint_at("S1", 0.0, 0.0, -50.0, 0.0),
                street_endpoint_at("S2", 0.0, 0.0, 0.0, 50.0),
            ],
        );
        let out = P30ActivityNodes.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["path_convergence"] - 1.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn isolated_plaza_with_no_nearby_streets_scores_zero_convergence() {
        let n = nbhd(vec![plaza_at("P1", 0.0, 0.0)], vec![street_endpoint_at("S1", 500.0, 500.0, 600.0, 500.0)]);
        let out = P30ActivityNodes.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, contributing_features, .. } => {
                assert!((sub_scores["path_convergence"] - 0.0).abs() < 1e-6);
                assert!(!contributing_features.is_empty());
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn two_plazas_spaced_near_the_alexander_target_score_high_spacing() {
        let n = nbhd(vec![plaza_at("P1", 0.0, 0.0), plaza_at("P2", 274.0, 0.0)], vec![]);
        let out = P30ActivityNodes.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!(sub_scores["spacing"] > 0.95, "got {}", sub_scores["spacing"]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn two_plazas_crammed_together_score_low_spacing() {
        let n = nbhd(vec![plaza_at("P1", 0.0, 0.0), plaza_at("P2", 20.0, 0.0)], vec![]);
        let out = P30ActivityNodes.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!(sub_scores["spacing"] < 0.2, "got {}", sub_scores["spacing"]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
