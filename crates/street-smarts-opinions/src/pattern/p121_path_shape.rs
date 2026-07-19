//! P121 Path Shape — a path should bulge in the middle and narrow at the
//! ends, forming a real place to linger, not just a straight line to move
//! through.
//!
//! From Alexander, *A Pattern Language*, Pattern 121 (p. 589), via
//! patternlanguage.cc/Patterns/Path-Shape-(121):
//! > **Problem:** Streets should be for staying in, and not just for
//! > moving through, the way they are today.
//! > **Solution:** Make a bulge in the middle of a public path, and make
//! > the ends narrower, so that the path forms an enclosure which is a
//! > place to stay, not just a place to pass through.
//!
//! # A structurally guaranteed gap, not a probabilistic one
//!
//! `path_network.rs`'s own module doc says it plainly: "Paths are
//! straight segments between block centroids... each individual edge does
//! not yet route around obstacles or follow shared boundaries." Every
//! `Street` this generator (and `p61_small_public_squares`'s connector
//! links) produces has exactly two centerline points -- start and end,
//! nothing between. A straight 2-point line has no midpoint to bulge BY
//! CONSTRUCTION, not by chance -- `bulge_fraction` (below) is
//! mathematically guaranteed to be 0.0 on any street this pipeline
//! currently generates, confirmed by the schema, not merely likely.
//!
//! # One real, checkable metric
//!
//! `bulge_fraction`: fraction of streets with 3+ centerline points (a
//! bulge is geometrically possible at all) whose maximum perpendicular
//! deviation of an interior point from the straight line between its
//! endpoints is at least the street's own `row_width_m` -- a real bulge
//! you'd notice, not rounding noise, scaled to the path's own width
//! rather than an arbitrary absolute distance.

use std::collections::BTreeMap;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P121PathShape;

/// Perpendicular distance in meters from `p` to the line through `a`-`b`,
/// using the same flat local-meter projection `p106_positive_outdoor_space`'s
/// own `hull` module uses.
fn perp_distance_m(p: &LngLat, a: &LngLat, b: &LngLat) -> f64 {
    let lat0 = a.lat;
    let mlat = lat0.to_radians().cos();
    let to_xy = |q: &LngLat| (q.lng * mlat * 111_320.0, q.lat * 110_540.0);
    let (ax, ay) = to_xy(a);
    let (bx, by) = to_xy(b);
    let (px, py) = to_xy(p);
    let dx = bx - ax;
    let dy = by - ay;
    let len2 = dx * dx + dy * dy;
    if len2 <= 0.0 {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    ((dy * (px - ax) - dx * (py - ay)).abs()) / len2.sqrt()
}

impl Opinion for P121PathShape {
    fn name(&self) -> &'static str {
        "p121_path_shape"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p121".into(),
            display: "Alexander et al., A Pattern Language, Pattern 121 (Path Shape)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Path-Shape-(121)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        if n.streets.is_empty() {
            return OpinionOutput::NoView {
                reason: "No streets in this neighborhood.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut bulging: Vec<bool> = Vec::new();
        let mut straight: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();
        let mut n_two_point = 0;

        for s in &n.streets {
            if s.centerline.len() < 3 {
                n_two_point += 1;
                bulging.push(false);
                straight.push(s.id.clone());
                continue;
            }
            let a = s.centerline.first().unwrap();
            let b = s.centerline.last().unwrap();
            let max_dev = s.centerline[1..s.centerline.len() - 1].iter()
                .map(|p| perp_distance_m(p, a, b))
                .fold(0.0_f64, f64::max);
            let width = s.row_width_m.unwrap_or(4.0).max(0.1);
            let has_bulge = max_dev >= width;
            bulging.push(has_bulge);
            details.insert(format!("{}.max_deviation_m", s.id), format!("{max_dev:.2}"));
            if !has_bulge {
                straight.push(s.id.clone());
            }
        }

        let value = bulging.iter().filter(|b| **b).count() as f64 / bulging.len() as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("bulge_fraction".into(), value);

        details.insert("n_streets".into(), n.streets.len().to_string());
        details.insert("n_two_point_straight_lines".into(), n_two_point.to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} street(s); {:.0}% have a real midpoint bulge (>= their own row width). {} \
                 street(s) are bare 2-point straight lines with no midpoint to bulge at all.",
                n.streets.len(), value * 100.0, n_two_point
            ),
            sub_scores,
            details,
            caveats: vec![
                "Every street this pipeline currently generates has exactly two centerline \
                 points (start and end) -- see this module's own doc comment. bulge_fraction is \
                 mathematically guaranteed to read 0.0 on real pipeline output today, not a \
                 possible-but-unlikely finding.".into(),
                "Doesn't check 'narrower at the ends' at all -- centerline geometry alone can't \
                 express varying path WIDTH along its length; row_width_m is one flat value per \
                 street.".into(),
            ],
            contributing_features: straight,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::nir::{NeighborhoodMeta, Street};

    fn nbhd(streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings: vec![], streets, open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P121 unit fixture".into(),
            },
        }
    }

    fn pt(x_m: f64, y_m: f64) -> LngLat {
        let m = 1.0 / 111_320.0;
        LngLat::new(x_m * m, y_m * m)
    }

    #[test]
    fn no_streets_is_no_view() {
        assert!(matches!(P121PathShape.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_bare_two_point_street_has_no_bulge() {
        // Matches the real generator's own current output exactly.
        let s = Street { id: "S1".into(), centerline: vec![pt(0.0, 0.0), pt(100.0, 0.0)], classification: Some("local".into()), row_width_m: Some(4.0) };
        let out = P121PathShape.evaluate(&nbhd(vec![s]));
        match out {
            OpinionOutput::Value { sub_scores, contributing_features, .. } => {
                assert!((sub_scores["bulge_fraction"] - 0.0).abs() < 1e-6);
                assert!(contributing_features.contains(&"S1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_dead_straight_three_point_street_has_no_bulge() {
        let s = Street { id: "S1".into(), centerline: vec![pt(0.0, 0.0), pt(50.0, 0.0), pt(100.0, 0.0)], classification: Some("local".into()), row_width_m: Some(4.0) };
        let out = P121PathShape.evaluate(&nbhd(vec![s]));
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["bulge_fraction"] - 0.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_real_wide_bulge_scores_full_credit() {
        // Midpoint offset 6m perpendicular on a 4m-wide path -- a real,
        // noticeable bulge exceeding the path's own width.
        let s = Street { id: "S1".into(), centerline: vec![pt(0.0, 0.0), pt(50.0, 6.0), pt(100.0, 0.0)], classification: Some("local".into()), row_width_m: Some(4.0) };
        let out = P121PathShape.evaluate(&nbhd(vec![s]));
        match out {
            OpinionOutput::Value { sub_scores, contributing_features, .. } => {
                assert!((sub_scores["bulge_fraction"] - 1.0).abs() < 1e-6);
                assert!(contributing_features.is_empty());
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
