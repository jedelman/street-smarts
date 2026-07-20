//! P124 Activity Pockets — a real plaza's edge should have real, small,
//! partly enclosed pockets of activity, not a blank frontage.
//!
//! From Alexander, *A Pattern Language*, Pattern 124, via
//! patternlanguage.cc/Patterns/Activity-Pockets-(124):
//! > **Problem:** The life of a public square forms naturally around its
//! > edge. If the edge fails, then the space never becomes lively.
//! > **Solution:** Surround public gathering places with pockets of
//! > activity -- small, partly enclosed areas at the edges, which jut
//! > forward into the open space between the paths, and contain
//! > activities which make it natural for people to pause and get
//! > involved.
//!
//! # A real, checkable proxy
//!
//! `p124_activity_pockets` (the generator, `street-smarts-patterns`) is
//! the only real producer of `OpenSpaceKind::Pocket` -- a real notch
//! carved from a bordering building's own footprint, open toward the
//! plaza, with a real `ActivityNode` at its centroid. This opinion checks,
//! for each real Plaza, whether at least one real Pocket sits within
//! `POCKET_ADJACENCY_M` of its own boundary. `value` = fraction of real
//! Plazas with at least one real, adjacent Pocket.
//!
//! This only checks whether a pocket EXISTS at a plaza's edge, not how
//! much of the edge it covers or how well it's enclosed -- Alexander's
//! own "surround" implies more than one pocket per real square in the
//! general case; a real, richer coverage measure is a possible future
//! refinement, not claimed here.

use std::collections::BTreeMap;
use street_smarts_core::geometry::point_to_polyline_m;
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P124ActivityPockets;

const POCKET_ADJACENCY_M: f64 = 3.0;

impl Opinion for P124ActivityPockets {
    fn name(&self) -> &'static str {
        "p124_activity_pockets"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p124".into(),
            display: "Alexander et al., A Pattern Language, Pattern 124 (Activity Pockets)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Activity-Pockets-(124)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let plazas: Vec<_> = n.open_space.iter().filter(|o| o.kind == OpenSpaceKind::Plaza).collect();
        if plazas.is_empty() {
            return OpinionOutput::NoView {
                reason: "No real Plaza-kind open space to check for adjacent pockets.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }
        let pockets: Vec<_> = n.open_space.iter().filter(|o| o.kind == OpenSpaceKind::Pocket).collect();

        let mut n_with_pocket = 0usize;
        for plaza in &plazas {
            let has_pocket = pockets.iter().any(|pocket| {
                pocket.polygon.outer.iter().any(|pt| point_to_polyline_m(pt, &plaza.polygon.outer) <= POCKET_ADJACENCY_M)
            });
            if has_pocket {
                n_with_pocket += 1;
            }
        }
        let value = n_with_pocket as f64 / plazas.len() as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("plazas_with_a_real_pocket".into(), value);

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_plazas".into(), plazas.len().to_string());
        details.insert("n_plazas_with_pocket".into(), n_with_pocket.to_string());
        details.insert("n_pockets_total".into(), pockets.len().to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} real plaza(s) checked; {} ({:.0}%) have at least one real Pocket within {POCKET_ADJACENCY_M:.0}m of their own boundary ({} real pocket(s) total).",
                plazas.len(), n_with_pocket, value * 100.0, pockets.len()
            ),
            sub_scores,
            details,
            caveats: vec![
                "Checks only whether a plaza has AT LEAST ONE real adjacent pocket, not how much \
                 of its edge is covered or how well-enclosed each pocket is -- Alexander's own \
                 'surround' implies more coverage than a single pocket in the general case.".into(),
                "p124_activity_pockets (generator) reads 'jut forward into the open space' as an \
                 alcove carved from a bordering building's own footprint -- a real, defensible, but \
                 not the only possible geometric interpretation of Alexander's text. See its own \
                 module doc.".into(),
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
    use street_smarts_core::nir::{NeighborhoodMeta, OpenSpace};

    fn m() -> f64 { 111_320.0 }

    fn square(id: &str, kind: OpenSpaceKind, cx: f64, cy: f64, half_side: f64) -> OpenSpace {
        let mm = m();
        let s = half_side / mm;
        let x = cx / mm;
        let y = cy / mm;
        OpenSpace {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(x - s, y - s), LngLat::new(x + s, y - s),
                LngLat::new(x + s, y + s), LngLat::new(x - s, y + s), LngLat::new(x - s, y - s),
            ]),
            kind,
        }
    }

    fn nbhd(open_space: Vec<OpenSpace>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings: vec![], streets: vec![], open_space,
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P124 unit fixture".into(),
            },
        }
    }

    #[test]
    fn no_plazas_is_no_view() {
        assert!(matches!(P124ActivityPockets.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_plaza_with_no_real_pocket_scores_zero() {
        let plz = square("PLZ1", OpenSpaceKind::Plaza, 0.0, 0.0, 15.0);
        let n = nbhd(vec![plz]);
        match P124ActivityPockets.evaluate(&n) {
            OpinionOutput::Value { value, details, .. } => {
                assert!((value - 0.0).abs() < 1e-9);
                assert_eq!(details["n_plazas_with_pocket"], "0");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_plaza_with_a_real_adjacent_pocket_scores_full_credit() {
        // Plaza spans x in [-15,15]; pocket right at its edge (x~15-17).
        let plz = square("PLZ1", OpenSpaceKind::Plaza, 0.0, 0.0, 15.0);
        let pocket = square("PLZ1_pocket_0", OpenSpaceKind::Pocket, 16.0, 0.0, 1.0);
        let n = nbhd(vec![plz, pocket]);
        match P124ActivityPockets.evaluate(&n) {
            OpinionOutput::Value { value, details, .. } => {
                assert!((value - 1.0).abs() < 1e-9);
                assert_eq!(details["n_plazas_with_pocket"], "1");
                assert_eq!(details["n_pockets_total"], "1");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_distant_pocket_does_not_count() {
        let plz = square("PLZ1", OpenSpaceKind::Plaza, 0.0, 0.0, 15.0);
        let pocket = square("FAR_pocket_0", OpenSpaceKind::Pocket, 500.0, 0.0, 1.0);
        let n = nbhd(vec![plz, pocket]);
        match P124ActivityPockets.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 0.0).abs() < 1e-9),
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
