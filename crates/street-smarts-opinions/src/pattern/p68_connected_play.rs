//! P68 Connected Play — common land connecting neighboring households
//! should form one real, continuous swath that children can move through
//! without crossing traffic.
//!
//! From Alexander, *A Pattern Language*, Pattern 68 (p. 341), via
//! patternlanguage.cc/Patterns/Connected-Play-(68):
//! > **Problem:** If children don't play enough with other children
//! > during the first five years of life, there is a great chance that
//! > they will have some kind of mental illness later in their lives.
//! > **Solution:** Lay out common land, paths, gardens, and bridges so
//! > that groups of at least 64 households are connected by a swath of
//! > land that does not cross traffic.
//!
//! # A real, checkable proxy -- connectivity, not the 64-household count
//!
//! Reuses `p67_common_land.rs`'s own real data source: `OpenSpace` tagged
//! `OpenSpaceKind::Common` (P37 House Cluster's per-block output). For
//! every pair of Common patches within `CONNECTION_THRESHOLD_M` (80m) of
//! each other, this opinion samples five points along the straight
//! segment between their centroids and checks that none sits within
//! `CROSSING_THRESHOLD_M` (8m, roughly a real street's own row width) of
//! an arterial street's centerline -- a real, computable "doesn't cross
//! traffic" check. `value` = fraction of nearby pairs connected this way.
//! Cannot check the real text's literal "64 households" threshold at all
//! -- no household-count concept exists in this schema, only parcels and
//! buildings.

use std::collections::BTreeMap;
use street_smarts_core::geometry::{haversine_m, point_to_polyline_m, LngLat};
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P68ConnectedPlay;

const CONNECTION_THRESHOLD_M: f64 = 80.0;
const CROSSING_THRESHOLD_M: f64 = 8.0;

fn crosses_arterial(a: &LngLat, b: &LngLat, arterials: &[&street_smarts_core::nir::Street]) -> bool {
    for i in 0..=4 {
        let t = i as f64 / 4.0;
        let p = LngLat::new(a.lng + (b.lng - a.lng) * t, a.lat + (b.lat - a.lat) * t);
        if arterials.iter().any(|s| point_to_polyline_m(&p, &s.centerline) <= CROSSING_THRESHOLD_M) {
            return true;
        }
    }
    false
}

impl Opinion for P68ConnectedPlay {
    fn name(&self) -> &'static str {
        "p68_connected_play"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p68".into(),
            display: "Alexander et al., A Pattern Language, Pattern 68 (Connected Play)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Connected-Play-(68)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let commons: Vec<_> = n.open_space.iter().filter(|o| o.kind == OpenSpaceKind::Common && o.polygon.area_m2() > 0.0).collect();
        if commons.len() < 2 {
            return OpinionOutput::NoView {
                reason: "Fewer than two Common open-space patches -- run P37 House Cluster first \
                         (need at least two blocks' worth of common land to check a connection)."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let arterials: Vec<_> = n.streets.iter().filter(|s| s.classification.as_deref() == Some("arterial")).collect();

        let mut pairs_checked = 0usize;
        let mut connected = 0usize;
        let mut cut: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for i in 0..commons.len() {
            for j in (i + 1)..commons.len() {
                let a = commons[i].polygon.centroid();
                let b = commons[j].polygon.centroid();
                let dist = haversine_m(&a, &b);
                if dist > CONNECTION_THRESHOLD_M {
                    continue;
                }
                pairs_checked += 1;
                let key = format!("{}<->{}", commons[i].id, commons[j].id);
                let blocked = crosses_arterial(&a, &b, &arterials);
                details.insert(format!("{key}.distance_m"), format!("{dist:.1}"));
                if blocked {
                    cut.push(key);
                } else {
                    connected += 1;
                }
            }
        }

        if pairs_checked == 0 {
            return OpinionOutput::NoView {
                reason: "No Common patches sit within the connection-distance threshold of each other.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let value = connected as f64 / pairs_checked as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("uncut_connection_fraction".into(), value);
        details.insert("n_common_patches".into(), commons.len().to_string());
        details.insert("n_nearby_pairs".into(), pairs_checked.to_string());
        details.insert("connection_threshold_m".into(), format!("{CONNECTION_THRESHOLD_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} Common patch(es), {} nearby pair(s) checked; {:.0}% connect without a real \
                 arterial street crossing between them.",
                commons.len(), pairs_checked, value * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Cannot check the real text's literal '64 households' threshold -- no household-\
                 count concept exists in this schema, only parcels and buildings.".into(),
                "Sampling five points along a straight centroid-to-centroid segment is a coarse \
                 stand-in for a real connected path -- doesn't confirm a walkable path actually \
                 exists along that line, only that a straight line between the two patches would \
                 clear traffic.".into(),
                "Only arterial-classified streets count as 'traffic' -- matches p59_quiet_backs.rs's \
                 own convention.".into(),
            ],
            contributing_features: cut,
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
                layer_provenance: Default::default(), label: "P68 unit fixture".into(),
            },
        }
    }

    fn common_patch(id: &str, cx: f64, cy: f64) -> OpenSpace {
        let m = 1.0 / 111_320.0;
        OpenSpace {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new((cx - 5.0) * m, (cy - 5.0) * m), LngLat::new((cx + 5.0) * m, (cy - 5.0) * m),
                LngLat::new((cx + 5.0) * m, (cy + 5.0) * m), LngLat::new((cx - 5.0) * m, (cy + 5.0) * m),
                LngLat::new((cx - 5.0) * m, (cy - 5.0) * m),
            ]),
            kind: OpenSpaceKind::Common,
        }
    }

    fn arterial_at(x_m: f64) -> Street {
        let m = 1.0 / 111_320.0;
        Street { id: "ART".into(), centerline: vec![LngLat::new(x_m * m, -200.0 * m), LngLat::new(x_m * m, 200.0 * m)], classification: Some("arterial".into()), row_width_m: Some(18.0) }
    }

    #[test]
    fn fewer_than_two_commons_is_no_view() {
        assert!(matches!(P68ConnectedPlay.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn two_nearby_commons_with_no_arterial_between_them_connect() {
        let n = nbhd(vec![common_patch("C1", 0.0, 0.0), common_patch("C2", 50.0, 0.0)], vec![]);
        match P68ConnectedPlay.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn an_arterial_between_two_commons_cuts_the_connection() {
        let n = nbhd(vec![common_patch("C1", 0.0, 0.0), common_patch("C2", 50.0, 0.0)], vec![arterial_at(25.0)]);
        match P68ConnectedPlay.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-6, "got {value}");
                assert!(!contributing_features.is_empty());
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
