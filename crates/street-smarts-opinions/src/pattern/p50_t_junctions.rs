//! P50 T Junctions — where two roads meet, they should meet in a clean
//! three-way T near 90 degrees, not a four-way crossing.
//!
//! From Alexander, *A Pattern Language*, Pattern 50 (p. 264), via
//! patternlanguage.cc/Patterns/T-Junctions-(50):
//! > **Problem:** Traffic accidents are far more frequent where two roads
//! > cross than at T Junctions.
//! > **Solution:** Lay out the road system so that any two roads which
//! > meet at grade, meet in three-way T junctions as near to 90 degrees as
//! > possible. Avoid four-way intersections and crossing movements.
//!
//! # Two real, checkable proxies
//!
//! Real intersections are found the same way `p49_looped_local_roads`
//! finds connected components: streets sharing a coordinate-snapped
//! endpoint. For each node where 3+ streets meet:
//!
//! - `three_way_fraction`: fraction of real intersections with EXACTLY
//!   three streets meeting there, not four or more -- directly checks
//!   "avoid four-way intersections."
//! - `near_90_fraction`: for three-way intersections only, whether the
//!   junction actually reads as a T -- among the three pairwise angles
//!   between the streets' outward direction vectors, the largest should
//!   be near 180° (the "through" road continuing straight) and the other
//!   two near 90° (the "stem" road meeting it square). Scored by how
//!   close the two non-through angles are to 90°, averaged.
//!
//! `value = 0.5 * three_way_fraction + 0.5 * near_90_fraction`.
//! `near_90_fraction` needs at least one three-way intersection to mean
//! anything; falls back to `three_way_fraction` alone otherwise.

use std::collections::BTreeMap;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P50TJunctions;

fn node_key(p: &LngLat) -> (i64, i64) {
    ((p.lng * 1e7).round() as i64, (p.lat * 1e7).round() as i64)
}

/// Angle in degrees between two 2D vectors (lng/lat treated as flat --
/// fine at the small local scale these intersections span).
fn angle_deg(a: (f64, f64), b: (f64, f64)) -> f64 {
    let dot = a.0 * b.0 + a.1 * b.1;
    let mag = ((a.0 * a.0 + a.1 * a.1).sqrt()) * ((b.0 * b.0 + b.1 * b.1).sqrt());
    if mag <= 0.0 {
        return 0.0;
    }
    (dot / mag).clamp(-1.0, 1.0).acos().to_degrees()
}

impl Opinion for P50TJunctions {
    fn name(&self) -> &'static str {
        "p50_t_junctions"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p50".into(),
            display: "Alexander et al., A Pattern Language, Pattern 50 (T Junctions)".into(),
            url: Some("https://patternlanguage.cc/Patterns/T-Junctions-(50)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        // For each node: list of outward direction vectors (one per
        // street touching it, direction from the node toward the street's
        // other end).
        let mut node_vectors: std::collections::HashMap<(i64, i64), Vec<(f64, f64)>> = std::collections::HashMap::new();
        for s in &n.streets {
            if s.centerline.len() < 2 {
                continue;
            }
            let a = s.centerline.first().unwrap();
            let b = s.centerline.last().unwrap();
            node_vectors.entry(node_key(a)).or_default().push((b.lng - a.lng, b.lat - a.lat));
            node_vectors.entry(node_key(b)).or_default().push((a.lng - b.lng, a.lat - b.lat));
        }

        let intersections: Vec<&Vec<(f64, f64)>> = node_vectors.values().filter(|v| v.len() >= 3).collect();
        if intersections.is_empty() {
            return OpinionOutput::NoView {
                reason: "No real intersections (3+ streets sharing an endpoint) in this neighborhood.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let three_way: Vec<&&Vec<(f64, f64)>> = intersections.iter().filter(|v| v.len() == 3).collect();
        let three_way_fraction = three_way.len() as f64 / intersections.len() as f64;

        let mut near_90_scores: Vec<f64> = Vec::new();
        for vectors in &three_way {
            let pairs = [(0, 1), (0, 2), (1, 2)];
            let angles: Vec<f64> = pairs.iter().map(|&(i, j)| angle_deg(vectors[i], vectors[j])).collect();
            let max_idx = angles.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).map(|(i, _)| i).unwrap();
            let stem_angles: Vec<f64> = angles.iter().enumerate().filter(|(i, _)| *i != max_idx).map(|(_, a)| *a).collect();
            let mean_deviation = stem_angles.iter().map(|a| (a - 90.0).abs()).sum::<f64>() / stem_angles.len() as f64;
            near_90_scores.push((1.0 - mean_deviation / 90.0).clamp(0.0, 1.0));
        }
        let near_90_fraction = if near_90_scores.is_empty() {
            None
        } else {
            Some(near_90_scores.iter().sum::<f64>() / near_90_scores.len() as f64)
        };

        let value = match near_90_fraction {
            Some(n90) => 0.5 * three_way_fraction + 0.5 * n90,
            None => three_way_fraction,
        };

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("three_way_fraction".into(), three_way_fraction);
        if let Some(n90) = near_90_fraction {
            sub_scores.insert("near_90_fraction".into(), n90);
        }

        let n_four_plus = intersections.len() - three_way.len();
        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_intersections".into(), intersections.len().to_string());
        details.insert("n_three_way".into(), three_way.len().to_string());
        details.insert("n_four_way_or_more".into(), n_four_plus.to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} real intersection(s); {:.0}% are clean three-way junctions ({} four-way-or-more).{}",
                intersections.len(), three_way_fraction * 100.0, n_four_plus,
                near_90_fraction.map(|n90| format!(" Mean near-90-degree score {n90:.2} among three-way junctions.")).unwrap_or_default(),
            ),
            sub_scores,
            details,
            caveats: vec![
                "Node identity is coordinate-snapping (1cm precision) on real street endpoints, \
                 not general line-crossing detection -- two streets that cross geometrically \
                 without sharing an endpoint vertex (e.g. an unmodeled grade separation) aren't \
                 counted as an intersection at all.".into(),
                "The 'through road' at each three-way junction is inferred as the pair of edges \
                 with the largest pairwise angle, not confirmed to actually be one continuous \
                 named road on both sides.".into(),
                "Angles are computed directly on raw lng/lat deltas with no cos(latitude) \
                 correction -- a real (if usually small at neighborhood scale) distortion away \
                 from the equator, since a degree of longitude is shorter than a degree of \
                 latitude at higher latitudes. Not corrected for here.".into(),
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
    use street_smarts_core::nir::{NeighborhoodMeta, Street};

    fn nbhd(streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings: vec![], streets, open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P50 unit fixture".into(),
            },
        }
    }

    fn street(id: &str, a: (f64, f64), b: (f64, f64)) -> Street {
        let m = 1.0 / 111_320.0;
        Street {
            id: id.into(),
            centerline: vec![LngLat::new(a.0 * m, a.1 * m), LngLat::new(b.0 * m, b.1 * m)],
            classification: Some("local".into()),
            row_width_m: Some(5.5), surface: None,
        }
    }

    #[test]
    fn no_intersections_is_no_view() {
        let n = nbhd(vec![street("S1", (0.0, 0.0), (100.0, 0.0))]);
        assert!(matches!(P50TJunctions.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_clean_perpendicular_t_scores_high() {
        // Through road along the x-axis, stem going straight up (+y) from
        // the shared origin -- a textbook 90-degree T.
        let n = nbhd(vec![
            street("THROUGH_A", (-100.0, 0.0), (0.0, 0.0)),
            street("THROUGH_B", (0.0, 0.0), (100.0, 0.0)),
            street("STEM", (0.0, 0.0), (0.0, 100.0)),
        ]);
        let out = P50TJunctions.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, value, .. } => {
                assert!((sub_scores["three_way_fraction"] - 1.0).abs() < 1e-6);
                assert!(sub_scores["near_90_fraction"] > 0.95, "got {}", sub_scores["near_90_fraction"]);
                assert!(value > 0.95, "got {value}");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_four_way_crossing_is_penalized() {
        let n = nbhd(vec![
            street("N", (0.0, 0.0), (0.0, 100.0)),
            street("S", (0.0, 0.0), (0.0, -100.0)),
            street("E", (0.0, 0.0), (100.0, 0.0)),
            street("W", (0.0, 0.0), (-100.0, 0.0)),
        ]);
        let out = P50TJunctions.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["three_way_fraction"] - 0.0).abs() < 1e-6, "a 4-way crossing should score zero three_way_fraction, got {}", sub_scores["three_way_fraction"]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_shallow_angle_t_scores_lower_than_90_degrees() {
        // Stem meets the through road at a shallow ~30-degree angle, not
        // a clean perpendicular T.
        let m = 1.0 / 111_320.0;
        let shallow = Street {
            id: "STEM".into(),
            centerline: vec![LngLat::new(0.0, 0.0), LngLat::new(50.0 * m, 28.9 * m)], // ~30 deg from x-axis
            classification: Some("local".into()), row_width_m: Some(5.5), surface: None,
        };
        let n = nbhd(vec![
            street("THROUGH_A", (-100.0, 0.0), (0.0, 0.0)),
            street("THROUGH_B", (0.0, 0.0), (100.0, 0.0)),
            shallow,
        ]);
        let out = P50TJunctions.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!(sub_scores["near_90_fraction"] < 0.8, "a ~30-degree stem should score noticeably below a clean 90, got {}", sub_scores["near_90_fraction"]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
