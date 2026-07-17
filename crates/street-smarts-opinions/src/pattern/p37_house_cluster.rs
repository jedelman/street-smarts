//! P37 House Cluster — did the raw parcel actually get carved into
//! human-scaled blocks with real reserved common land, or did it produce
//! one flat field with no cluster identity?
//!
//! From Alexander, *A Pattern Language*, Pattern 37:
//! > People will not feel that they have a stake in their own house
//! > cluster, or take responsibility for it, unless the cluster itself is
//! > a distinct, identifiable place, separated from other clusters by
//! > common land.
//!
//! # What this checks
//! Two things, matching the pattern's own two claims: (1) blocks exist at
//! all (`BLOCK_n` parcels present -- a run that silently produced zero
//! blocks would otherwise only surface as every downstream operator
//! quietly having nothing to do); (2) common land coverage -- the
//! generator names each block's reserved common land `{block_id}_common`
//! when it succeeds (skipping only when the resulting patch would be
//! smaller than `min_common_land_area_m2`, a legitimate "too small to be
//! worth reserving" case, not a bug) -- so this opinion checks what
//! fraction of blocks have that companion entity present.

use std::collections::BTreeMap;
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;
use street_smarts_core::Scope;

pub struct P37HouseCluster;

impl Opinion for P37HouseCluster {
    fn name(&self) -> &'static str {
        "p37_house_cluster"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p37".into(),
            display: "Alexander et al., A Pattern Language, Pattern 37 (House Cluster)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl37/apl37.htm".into()),
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

        let common_land_ids: std::collections::HashSet<&str> = n
            .open_space
            .iter()
            .filter(|o| o.kind == OpenSpaceKind::Common)
            .map(|o| o.id.as_str())
            .collect();

        let mut n_with_common_land = 0usize;
        let mut without: Vec<String> = Vec::new();
        for block in &blocks {
            let expected_id = format!("{}_common", block.id);
            if common_land_ids.contains(expected_id.as_str()) {
                n_with_common_land += 1;
            } else {
                without.push(block.id.clone());
            }
        }

        let common_land_fraction = n_with_common_land as f64 / blocks.len() as f64;
        // Block existence alone is worth half credit -- a run with zero
        // common land anywhere is still a real, if incomplete, cluster
        // carve, not a total failure the way zero blocks would be.
        let value = 0.5 + 0.5 * common_land_fraction;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("common_land_fraction".into(), common_land_fraction);

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_blocks".into(), blocks.len().to_string());
        details.insert("n_with_common_land".into(), n_with_common_land.to_string());
        details.insert("n_common_land_entities_total".into(), common_land_ids.len().to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} block(s) carved; {}/{} have their own reserved common land.",
                blocks.len(),
                n_with_common_land,
                blocks.len()
            ),
            sub_scores,
            details,
            caveats: vec![
                "A block legitimately has no common land when \
                 common_land_fraction * block_area < min_common_land_area_m2 (too small a patch \
                 to be worth reserving) -- this opinion can't distinguish that intentional skip \
                 from a real bug that dropped the common land some other way; a low fraction is \
                 worth a second look, not an automatic failure."
                    .into(),
                "Doesn't check block SIZE distribution (human-scaled vs. accidentally huge or \
                 sliver blocks) or common land's actual shape/usability, only presence."
                    .into(),
            ],
            contributing_features: without,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{NeighborhoodMeta, OpenSpace, Parcel};

    fn nbhd(parcels: Vec<Parcel>, open_space: Vec<OpenSpace>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels,
            buildings: vec![],
            streets: vec![],
            open_space,
            boundaries: vec![],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P37 opinion fixture".into(),
            },
        }
    }

    fn block(id: &str) -> Parcel {
        Parcel {
            id: id.into(),
            polygon: Polygon::from_ring(vec![]),
            area_acres: 0.0,
            use_category: None,
            ownership: None,
            is_eda: false,
            spec: Some(format!("BLOCK_{id}")),
            density_tier: None,
            target_stories: None,
        }
    }

    fn common_land(block_id: &str) -> OpenSpace {
        OpenSpace {
            id: format!("{block_id}_common"),
            polygon: Polygon::from_ring(vec![]),
            kind: OpenSpaceKind::Common,
        }
    }

    #[test]
    fn no_blocks_is_no_view() {
        assert!(matches!(P37HouseCluster.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn blocks_with_no_common_land_score_half() {
        let n = nbhd(vec![block("0"), block("1")], vec![]);
        match P37HouseCluster.evaluate(&n) {
            OpinionOutput::Value { value, sub_scores, .. } => {
                assert!((value - 0.5).abs() < 1e-9, "got {value}");
                assert!(sub_scores["common_land_fraction"].abs() < 1e-9);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn full_common_land_coverage_scores_full() {
        let n = nbhd(
            vec![block("0"), block("1")],
            vec![common_land("0"), common_land("1")],
        );
        match P37HouseCluster.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 1.0).abs() < 1e-9, "got {value}");
                assert!(contributing_features.is_empty());
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn partial_coverage_lists_the_missing_blocks() {
        let n = nbhd(vec![block("0"), block("1"), block("2")], vec![common_land("0")]);
        match P37HouseCluster.evaluate(&n) {
            OpinionOutput::Value { sub_scores, contributing_features, .. } => {
                assert!((sub_scores["common_land_fraction"] - 1.0 / 3.0).abs() < 1e-9);
                assert_eq!(contributing_features, vec!["1".to_string(), "2".to_string()]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
