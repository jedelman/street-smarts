//! P126 Something Roughly in the Middle — every public square needs a
//! real focal object roughly (not exactly) at its center, giving it a
//! pulse that draws people in.
//!
//! From Alexander, *A Pattern Language*, Pattern 126 (p. 606), via
//! patternlanguage.cc/Patterns/Something-Roughly-in-the-Middle-(126):
//! > **Problem:** A public space without a middle is quite likely to stay
//! > empty.
//! > **Solution:** Between the natural paths which cross a public square
//! > or courtyard or a piece of common land, choose something to stand
//! > roughly in the middle... Leave it exactly where it falls between the
//! > paths; resist the impulse to put it exactly in the middle.
//!
//! # A real, checkable proxy -- honestly `NoView` on real pipeline output
//! today
//!
//! `Neighborhood.activity_nodes: Vec<ActivityNode>` is a real, typed,
//! serializable field -- but as of this writing no generator anywhere in
//! this pipeline populates it, the same class of real-field-no-producer
//! gap as `Parcel.use_category: "commercial"` (see `p33_night_life.rs`'s
//! own module doc) and `Neighborhood.boundaries` (`p53_main_gateways.rs`).
//! For each real `OpenSpaceKind::Plaza`, this opinion checks whether at
//! least one `ActivityNode` sits within `MIDDLE_RADIUS_FRACTION` (0.6) of
//! the plaza's own bounding-box short side from its centroid -- close to
//! the middle without requiring exact centering, matching Alexander's own
//! "roughly, not exactly" instruction. `value` = fraction of plazas with
//! a real, roughly-central activity node. Honestly `NoView` on every real
//! pipeline output today, purely because `activity_nodes` is always
//! empty -- not a detector bug.

use std::collections::BTreeMap;
use street_smarts_core::geometry::{haversine_m, LngLat};
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P126SomethingRoughlyInTheMiddle;

const MIDDLE_RADIUS_FRACTION: f64 = 0.6;

fn bbox_short_side_m(ring: &[LngLat]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let lat0 = ring.iter().map(|p| p.lat).sum::<f64>() / ring.len() as f64;
    let mlat = lat0.to_radians().cos();
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in ring {
        let x = p.lng * mlat * 111_320.0;
        let y = p.lat * 110_540.0;
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    (max_x - min_x).min(max_y - min_y)
}

impl Opinion for P126SomethingRoughlyInTheMiddle {
    fn name(&self) -> &'static str {
        "p126_something_roughly_in_the_middle"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p126".into(),
            display: "Alexander et al., A Pattern Language, Pattern 126 (Something Roughly in the Middle)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Something-Roughly-in-the-Middle-(126)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let plazas: Vec<_> = n.open_space.iter().filter(|o| o.kind == OpenSpaceKind::Plaza && o.polygon.area_m2() > 0.0).collect();
        if plazas.is_empty() {
            return OpinionOutput::NoView {
                reason: "No real Plaza open space in this neighborhood -- run P61 Small Public Squares first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }
        if n.activity_nodes.is_empty() {
            return OpinionOutput::NoView {
                reason: "No generator populates Neighborhood.activity_nodes yet -- there is nothing \
                         to check against a plaza's own middle. See this opinion's own module doc."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut has_middle = 0usize;
        let mut empty: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for p in &plazas {
            let centroid = p.polygon.centroid();
            let radius = bbox_short_side_m(&p.polygon.outer) * MIDDLE_RADIUS_FRACTION;
            let nearest_m = n.activity_nodes.iter()
                .map(|a| haversine_m(&centroid, &a.location))
                .fold(f64::MAX, f64::min);
            let ok = nearest_m <= radius;
            details.insert(format!("{}.nearest_activity_node_m", p.id), format!("{:.1}", nearest_m.min(9999.0)));
            if ok {
                has_middle += 1;
            } else {
                empty.push(p.id.clone());
            }
        }

        let value = has_middle as f64 / plazas.len() as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("has_middle_fraction".into(), value);
        details.insert("n_plazas".into(), plazas.len().to_string());
        details.insert("middle_radius_fraction".into(), format!("{MIDDLE_RADIUS_FRACTION:.2}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} real plaza(s) checked; {:.0}% have a real activity node within {:.0}% of the \
                 plaza's own short side of its centroid.",
                plazas.len(), value * 100.0, MIDDLE_RADIUS_FRACTION * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Doesn't verify the object is 'between the natural paths' that cross the square -- \
                 no path-crossing-a-plaza concept is checked here, only proximity to the centroid.".into(),
                "Any ActivityKind counts equally -- doesn't distinguish a fountain/statue/bandstand \
                 from, say, a Commerce node that happens to sit near a plaza's middle.".into(),
            ],
            contributing_features: empty,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{ActivityKind, ActivityNode, NeighborhoodMeta, OpenSpace};

    fn nbhd(open_space: Vec<OpenSpace>, activity_nodes: Vec<ActivityNode>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings: vec![], streets: vec![], open_space,
            boundaries: vec![], activity_nodes,
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P126 unit fixture".into(),
            },
        }
    }

    fn plaza(id: &str, side_m: f64) -> OpenSpace {
        let m = 1.0 / 111_320.0;
        let half = side_m / 2.0 * m;
        OpenSpace {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(-half, -half), LngLat::new(half, -half),
                LngLat::new(half, half), LngLat::new(-half, half), LngLat::new(-half, -half),
            ]),
            kind: OpenSpaceKind::Plaza,
        }
    }

    #[test]
    fn no_plazas_is_no_view() {
        assert!(matches!(P126SomethingRoughlyInTheMiddle.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn no_activity_nodes_is_no_view() {
        let n = nbhd(vec![plaza("PZ1", 30.0)], vec![]);
        assert!(matches!(P126SomethingRoughlyInTheMiddle.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_node_near_the_plazas_middle_scores_full() {
        let node = ActivityNode { id: "A1".into(), location: LngLat::new(0.0, 0.0), kind: ActivityKind::Civic, intensity: None, label: None };
        let n = nbhd(vec![plaza("PZ1", 30.0)], vec![node]);
        match P126SomethingRoughlyInTheMiddle.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_far_node_scores_zero() {
        let m = 1.0 / 111_320.0;
        let node = ActivityNode { id: "A1".into(), location: LngLat::new(500.0 * m, 500.0 * m), kind: ActivityKind::Civic, intensity: None, label: None };
        let n = nbhd(vec![plaza("PZ1", 30.0)], vec![node]);
        match P126SomethingRoughlyInTheMiddle.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-6, "got {value}");
                assert!(contributing_features.contains(&"PZ1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
