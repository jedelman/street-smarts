//! P29 Density Rings — did density actually vary by distance from the
//! site's center, or is it flat everywhere?
//!
//! From Alexander, *A Pattern Language*, Pattern 29:
//! > Arrange the housing so that it steps down in density, from the
//! > center of the town...
//!
//! # What this checks
//! Every `BLOCK_n` parcel `p29_density_rings` has tagged carries a
//! `density_tier` label and a `target_stories` number. This opinion checks
//! two things: coverage (what fraction of blocks got tagged at all -- a
//! block a later operator replaced without carrying the tag forward would
//! show up here) and variance (Alexander's actual claim isn't "blocks have
//! a tier," it's that density *steps down* -- so a run where every block
//! ended up in the same tier, or with the same `target_stories`, technically
//! has coverage but hasn't actually produced the gradient the pattern
//! describes). `value` rewards both; `variance_fraction` sub-score isolates
//! the more interesting, contested half of the claim.

use std::collections::BTreeMap;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;
use street_smarts_core::Scope;

pub struct P29DensityRings;

impl Opinion for P29DensityRings {
    fn name(&self) -> &'static str {
        "p29_density_rings"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p29".into(),
            display: "Alexander et al., A Pattern Language, Pattern 29 (Density Rings)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl29/apl29.htm".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let blocks: Vec<_> = n.select(&Scope::Block).collect();
        if blocks.is_empty() {
            return OpinionOutput::NoView {
                reason: "No BLOCK_n parcels found -- run p37_house_cluster first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let tagged: Vec<_> = blocks.iter().filter(|p| p.density_tier.is_some()).collect();
        if tagged.is_empty() {
            return OpinionOutput::NoView {
                reason: "Blocks exist but none carry a density_tier -- run \
                         p29_density_rings first."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let coverage = tagged.len() as f64 / blocks.len() as f64;

        let distinct_tiers: std::collections::HashSet<&str> =
            tagged.iter().filter_map(|p| p.density_tier.as_deref()).collect();
        let distinct_targets: std::collections::HashSet<u64> = tagged
            .iter()
            .filter_map(|p| p.target_stories.map(|t| t.to_bits()))
            .collect();
        // A real gradient needs more than one tier actually represented --
        // one tier (however many blocks) is a flat site, not a step-down.
        let variance_fraction = if tagged.len() > 1 && distinct_tiers.len() > 1 { 1.0 } else { 0.0 };

        let value = (coverage + variance_fraction) / 2.0;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("coverage".into(), coverage);
        sub_scores.insert("variance_fraction".into(), variance_fraction);

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_blocks".into(), blocks.len().to_string());
        details.insert("n_tagged".into(), tagged.len().to_string());
        details.insert("n_distinct_tiers".into(), distinct_tiers.len().to_string());
        details.insert("n_distinct_target_stories".into(), distinct_targets.len().to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{}/{} blocks tagged with a density tier; {} distinct tier(s) present ({}).",
                tagged.len(),
                blocks.len(),
                distinct_tiers.len(),
                if distinct_tiers.len() > 1 { "a real gradient" } else { "flat -- no step-down" }
            ),
            sub_scores,
            details,
            caveats: vec![
                "Only checks that tiers/targets vary across blocks, not that the variation \
                 correlates sensibly with actual distance from a real town/site center -- a \
                 shuffled assignment would still score well here."
                    .into(),
                "target_stories is a block-level GOAL, not a promise every building on that \
                 block ends up that height -- see p96_number_of_stories for the per-pad reality."
                    .into(),
            ],
            contributing_features: blocks
                .iter()
                .filter(|p| p.density_tier.is_none())
                .map(|p| p.id.clone())
                .collect(),
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{NeighborhoodMeta, Parcel};

    fn nbhd(parcels: Vec<Parcel>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.001, 0.001],
            parcels,
            buildings: vec![],
            streets: vec![],
            open_space: vec![],
            boundaries: vec![],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P29 opinion fixture".into(),
            },
        }
    }

    fn block(id: &str, tier: Option<&str>, target: Option<f64>) -> Parcel {
        Parcel {
            id: id.into(),
            polygon: Polygon::from_ring(vec![]),
            area_acres: 0.0,
            use_category: None,
            ownership: None,
            is_eda: false,
            spec: Some(format!("BLOCK_{id}")),
            density_tier: tier.map(String::from),
            target_stories: target,
        }
    }

    #[test]
    fn no_blocks_is_no_view() {
        assert!(matches!(P29DensityRings.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn untagged_blocks_is_no_view() {
        let n = nbhd(vec![block("0", None, None), block("1", None, None)]);
        assert!(matches!(P29DensityRings.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn uniform_tier_scores_low_variance_despite_full_coverage() {
        let n = nbhd(vec![
            block("0", Some("core"), Some(4.0)),
            block("1", Some("core"), Some(4.0)),
        ]);
        match P29DensityRings.evaluate(&n) {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["coverage"] - 1.0).abs() < 1e-9);
                assert!(sub_scores["variance_fraction"].abs() < 1e-9);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn varied_tiers_score_full_variance() {
        // "core"/"ring_1"/"edge" -- the real vocabulary
        // p29_density_rings::P29DensityRings actually produces (ring_idx
        // 0/1/n_rings-1 at the n_rings=3 default). "mid" was here before
        // and is not a value the real generator ever emits -- caught
        // while building the typed DensityTier component and cross-
        // checking against the generator's actual tier-labeling logic.
        let n = nbhd(vec![
            block("0", Some("core"), Some(4.0)),
            block("1", Some("ring_1"), Some(3.0)),
            block("2", Some("edge"), Some(2.0)),
        ]);
        match P29DensityRings.evaluate(&n) {
            OpinionOutput::Value { value, sub_scores, .. } => {
                assert!((sub_scores["coverage"] - 1.0).abs() < 1e-9);
                assert!((sub_scores["variance_fraction"] - 1.0).abs() < 1e-9);
                assert!((value - 1.0).abs() < 1e-9);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn partial_coverage_is_reflected_and_listed() {
        let n = nbhd(vec![
            block("0", Some("core"), Some(4.0)),
            block("1", Some("edge"), Some(2.0)),
            block("2", None, None),
        ]);
        match P29DensityRings.evaluate(&n) {
            OpinionOutput::Value { sub_scores, contributing_features, .. } => {
                assert!((sub_scores["coverage"] - (2.0 / 3.0)).abs() < 1e-9);
                assert_eq!(contributing_features, vec!["2".to_string()]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
