//! P67 Common Land — every house cluster needs a real, generous, shared
//! piece of land its households actually use, not a token gesture.
//!
//! From Alexander, *A Pattern Language*, Pattern 67 (Oxford University
//! Press, 1977, p. 336), via patternlanguage.cc/Patterns/Common-Land-(67):
//! > **Problem:** Without common land no social system can survive.
//! > **Solution:** Give over 25 percent of the land in house clusters to
//! > common land which touches, or is very near, the homes which share it.
//! > Basic: be wary of the automobile; on no account let it dominate this
//! > land.
//!
//! # A real gap this opinion found, and closed
//!
//! `p37_house_cluster.rs`'s own `common_land_fraction` parameter originally
//! DEFAULTED TO 0.12 (12%) -- well under half of Alexander's literal "25
//! percent" figure. This opinion is what found that gap: an independent
//! check against Alexander's own number, not the generator's own doc
//! comment claiming "this produces Common Land." The generator's default
//! has since been raised to 0.26 (max stays 0.4) specifically to clear
//! this check -- see `p37_house_cluster.rs`'s own module doc for the fix.
//! This opinion still scores against Alexander's literal number, not
//! whatever the generator's current default happens to be, so it stays a
//! real check rather than a tautology.
//!
//! # Two sub-scores
//!
//! - `area_fraction`: for each `BLOCK_n` parcel, the real area of
//!   `OpenSpace` tagged `OpenSpaceKind::Common` with id `"{block_id}_common"`
//!   (P37's own naming convention -- see its module doc) divided by the
//!   block's own area, normalized against the literal 0.25 threshold
//!   (`min(1.0, fraction / 0.25)`), area-weighted across blocks.
//! - `proximity`: whether each block's common land actually sits near real
//!   buildings, not just floating in an empty block -- distance from each
//!   common-land patch's centroid to the nearest building anywhere in the
//!   neighborhood, scored 1.0 within `near_threshold_m` (default 60m,
//!   P61's own "a few dozen meters" scale) and falling off linearly to 0 by
//!   `2 * near_threshold_m`.
//!
//! `value = 0.5 * area_fraction + 0.5 * proximity`, matching
//! `p106_positive_outdoor_space`'s own two-independent-checks convention.

use std::collections::BTreeMap;
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;
use street_smarts_core::Scope;

pub struct P67CommonLand;

const ALEXANDER_TARGET_FRACTION: f64 = 0.25;
const NEAR_THRESHOLD_M: f64 = 60.0;

impl Opinion for P67CommonLand {
    fn name(&self) -> &'static str {
        "p67_common_land"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p67".into(),
            display: "Alexander et al., A Pattern Language, Pattern 67 (Common Land)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Common-Land-(67)".into()),
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
                reason: "No BLOCK_n parcels in this neighborhood -- run P37 House Cluster first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut total_block_area = 0.0;
        let mut fraction_weighted_sum = 0.0;
        let mut proximity_scores: Vec<f64> = Vec::new();
        let mut blocks_with_no_common_land: Vec<String> = Vec::new();
        let mut under_threshold: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for block in &blocks {
            let block_area = block.polygon.area_m2();
            if block_area <= 0.0 {
                continue;
            }
            total_block_area += block_area;

            let common_id = format!("{}_common", block.id);
            let common_patch = n.open_space.iter().find(|o| o.id == common_id && o.kind == OpenSpaceKind::Common);

            let fraction = match common_patch {
                Some(patch) => patch.polygon.area_m2() / block_area,
                None => {
                    blocks_with_no_common_land.push(block.id.clone());
                    0.0
                }
            };
            fraction_weighted_sum += (fraction / ALEXANDER_TARGET_FRACTION).min(1.0) * block_area;
            details.insert(format!("{}.common_fraction", block.id), format!("{:.2}", fraction));
            if fraction < ALEXANDER_TARGET_FRACTION {
                under_threshold.push(block.id.clone());
            }

            if let Some(patch) = common_patch {
                if n.buildings.is_empty() {
                    continue;
                }
                let patch_centroid = patch.polygon.centroid();
                let nearest_m = n.buildings.iter()
                    .map(|b| haversine_m(&patch_centroid, &b.polygon.centroid()))
                    .fold(f64::MAX, f64::min);
                let score = (1.0 - (nearest_m - NEAR_THRESHOLD_M).max(0.0) / NEAR_THRESHOLD_M).clamp(0.0, 1.0);
                proximity_scores.push(score);
            }
        }

        if total_block_area <= 0.0 {
            return OpinionOutput::NoView {
                reason: "Blocks present but none had positive real area to score.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let area_fraction = fraction_weighted_sum / total_block_area;
        let proximity = if proximity_scores.is_empty() {
            None
        } else {
            Some(proximity_scores.iter().sum::<f64>() / proximity_scores.len() as f64)
        };
        let value = match proximity {
            Some(p) => 0.5 * area_fraction + 0.5 * p,
            // No buildings yet to check proximity against (P37 ran, P95/P107
            // haven't) -- score on area_fraction alone rather than
            // penalizing a pipeline stage that hasn't run yet.
            None => area_fraction,
        };

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("area_fraction".into(), area_fraction);
        if let Some(p) = proximity {
            sub_scores.insert("proximity".into(), p);
        }

        details.insert("n_blocks".into(), blocks.len().to_string());
        details.insert("n_blocks_no_common_land".into(), blocks_with_no_common_land.len().to_string());
        details.insert("n_blocks_under_25pct".into(), under_threshold.len().to_string());
        details.insert("target_fraction".into(), format!("{:.2}", ALEXANDER_TARGET_FRACTION));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} block(s), area-weighted mean common-land fraction {:.1}% (Alexander's target: {:.0}%); {} block(s) have no common land at all.{}",
                blocks.len(),
                area_fraction * ALEXANDER_TARGET_FRACTION * 100.0,
                ALEXANDER_TARGET_FRACTION * 100.0,
                blocks_with_no_common_land.len(),
                proximity.map(|p| format!(" Mean proximity-to-buildings score {:.2}.", p)).unwrap_or_default(),
            ),
            sub_scores,
            details,
            caveats: vec![
                "area_fraction is normalized against Alexander's literal '25 percent' figure, not \
                 whatever p37_house_cluster's own common_land_fraction default happens to be -- if \
                 that default is ever lowered again, this opinion will score it below 1.0 by \
                 design, not by opinion error. See this module's own doc comment.".into(),
                "Common land is matched to its source block by id convention \
                 (\"{block_id}_common\", P37's own naming) -- an OpenSpace tagged Common that a \
                 future operator names differently won't be found by this opinion.".into(),
                "proximity is a straight-line nearest-building distance, not a real walkability/\
                 visibility check, and doesn't confirm the nearby building actually belongs to the \
                 SAME cluster (exact block membership after P108's merges needs the ledger's \
                 block_membership resolution, not attempted here -- see components.rs's own doc \
                 comment on why that's hard). A building from an adjacent block sitting close by \
                 can inflate this score.".into(),
                "Doesn't check Alexander's second sentence at all (\"be wary of the automobile... \
                 on no account let it dominate this land\") -- no vehicular-access modeling exists \
                 in this schema to check against.".into(),
            ],
            contributing_features: under_threshold,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{Building, NeighborhoodMeta, OpenSpace, Parcel};

    fn nbhd(parcels: Vec<Parcel>, open_space: Vec<OpenSpace>, buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels,
            buildings,
            streets: vec![],
            open_space,
            boundaries: vec![],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P67 unit fixture".into(),
            },
        }
    }

    /// A 100m x 100m square block (10,000 m²) at a given origin, `BLOCK_n` spec.
    fn block(id: &str, lng0: f64, lat0: f64) -> Parcel {
        let m = 1.0 / 111_320.0;
        Parcel {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(lng0, lat0),
                LngLat::new(lng0 + 100.0 * m, lat0),
                LngLat::new(lng0 + 100.0 * m, lat0 + 100.0 * m),
                LngLat::new(lng0, lat0 + 100.0 * m),
                LngLat::new(lng0, lat0),
            ]),
            area_acres: 2.47,
            use_category: None,
            ownership: None,
            is_eda: true,
            spec: Some("BLOCK_1".into()),
            density_tier: None,
            target_stories: None,
        }
    }

    /// A square patch of `side_m` centered at `(lng0 + 50m, lat0 + 50m)`.
    fn common_patch(id: &str, lng0: f64, lat0: f64, side_m: f64) -> OpenSpace {
        let m = 1.0 / 111_320.0;
        let half = side_m / 2.0;
        let cx = lng0 + 50.0 * m;
        let cy = lat0 + 50.0 * m;
        OpenSpace {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(cx - half * m, cy - half * m),
                LngLat::new(cx + half * m, cy - half * m),
                LngLat::new(cx + half * m, cy + half * m),
                LngLat::new(cx - half * m, cy + half * m),
                LngLat::new(cx - half * m, cy - half * m),
            ]),
            kind: OpenSpaceKind::Common,
        }
    }

    #[test]
    fn no_blocks_is_no_view() {
        let n = nbhd(vec![], vec![], vec![]);
        let out = P67CommonLand.evaluate(&n);
        assert!(matches!(out, OpinionOutput::NoView { .. }));
    }

    #[test]
    fn block_with_no_common_land_scores_zero_area_fraction() {
        let n = nbhd(vec![block("BLOCK_1", 0.0, 0.0)], vec![], vec![]);
        let out = P67CommonLand.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, contributing_features, .. } => {
                assert!((sub_scores["area_fraction"] - 0.0).abs() < 1e-6);
                assert!(contributing_features.contains(&"BLOCK_1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn block_meeting_alexanders_25_percent_scores_full_area_fraction() {
        // 10,000 m² block, 2,500 m² common land = exactly 25%.
        let n = nbhd(
            vec![block("BLOCK_1", 0.0, 0.0)],
            vec![common_patch("BLOCK_1_common", 0.0, 0.0, 50.0)],
            vec![],
        );
        let out = P67CommonLand.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, value, .. } => {
                assert!((sub_scores["area_fraction"] - 1.0).abs() < 0.02, "got {}", sub_scores["area_fraction"]);
                // No buildings yet -- value should equal area_fraction alone.
                assert!((value - sub_scores["area_fraction"]).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_fraction_below_alexanders_target_still_scores_proportionally() {
        // 10,000 m² block, 1,200 m² common land -- 12%, once p37's own
        // default before it was raised to close this exact gap (see
        // p37_house_cluster.rs's own module doc). Kept as a boundary-case
        // regression: a real generator regressing back toward a low
        // fraction should still score well below full credit.
        let side = (1200.0_f64).sqrt();
        let n = nbhd(
            vec![block("BLOCK_1", 0.0, 0.0)],
            vec![common_patch("BLOCK_1_common", 0.0, 0.0, side)],
            vec![],
        );
        let out = P67CommonLand.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                let expected = 0.12 / ALEXANDER_TARGET_FRACTION;
                assert!(
                    (sub_scores["area_fraction"] - expected).abs() < 0.02,
                    "expected ~{expected:.2} (12%/25%), got {}", sub_scores["area_fraction"]
                );
                assert!(sub_scores["area_fraction"] < 0.5, "12% common land should score well below half of Alexander's target");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn generators_own_26_percent_default_clears_alexanders_target() {
        // Matches p37_house_cluster's own common_land_fraction default
        // (0.26, raised specifically to clear Alexander's literal "over 25
        // percent" after this opinion found the original 0.12 default
        // failing it -- see p37_house_cluster.rs's own module doc).
        // 10,000 m² block, 2,600 m² common land.
        let side = (2600.0_f64).sqrt();
        let n = nbhd(
            vec![block("BLOCK_1", 0.0, 0.0)],
            vec![common_patch("BLOCK_1_common", 0.0, 0.0, side)],
            vec![],
        );
        let out = P67CommonLand.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!(
                    (sub_scores["area_fraction"] - 1.0).abs() < 1e-6,
                    "26% common land should clear Alexander's 25% target and clamp to full credit, got {}",
                    sub_scores["area_fraction"]
                );
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn common_land_near_a_real_building_scores_high_proximity() {
        let m = 1.0 / 111_320.0;
        let building = Building {
            id: "B1".into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(60.0 * m, 60.0 * m),
                LngLat::new(70.0 * m, 60.0 * m),
                LngLat::new(70.0 * m, 70.0 * m),
                LngLat::new(60.0 * m, 70.0 * m),
                LngLat::new(60.0 * m, 60.0 * m),
            ]),
            height_m: Some(9.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: None,
            openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        };
        let n = nbhd(
            vec![block("BLOCK_1", 0.0, 0.0)],
            vec![common_patch("BLOCK_1_common", 0.0, 0.0, 50.0)],
            vec![building],
        );
        let out = P67CommonLand.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!(sub_scores["proximity"] > 0.9, "a building ~15m from the common-land centroid should score near-full proximity, got {}", sub_scores["proximity"]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn common_land_far_from_every_building_scores_low_proximity() {
        let m = 1.0 / 111_320.0;
        let far_building = Building {
            id: "B_FAR".into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(1000.0 * m, 1000.0 * m),
                LngLat::new(1010.0 * m, 1000.0 * m),
                LngLat::new(1010.0 * m, 1010.0 * m),
                LngLat::new(1000.0 * m, 1010.0 * m),
                LngLat::new(1000.0 * m, 1000.0 * m),
            ]),
            height_m: Some(9.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: None,
            openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        };
        let n = nbhd(
            vec![block("BLOCK_1", 0.0, 0.0)],
            vec![common_patch("BLOCK_1_common", 0.0, 0.0, 50.0)],
            vec![far_building],
        );
        let out = P67CommonLand.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!(sub_scores["proximity"] < 0.1, "a building ~1.3km away should score near-zero proximity, got {}", sub_scores["proximity"]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
