//! P52 Network of Paths and Cars — pedestrian paths should cross roads
//! perpendicularly, forming their own network, not run alongside the road
//! system.
//!
//! From Alexander, *A Pattern Language*, Pattern 52 (p. 270), via
//! patternlanguage.cc/Patterns/Network-of-Paths-and-Cars-(52):
//! > **Problem:** Cars are dangerous to pedestrians; yet activities occur
//! > just where cars and pedestrians meet.
//! > **Solution:** Establish pedestrian paths that cross roads
//! > perpendicularly rather than running alongside them... adding paths
//! > one at a time through the center of blocks so they traverse roads
//! > orthogonally.
//!
//! # A real, checkable proxy
//!
//! `path_network.rs` (this pipeline's real generator for this pattern,
//! despite the file not being named `p52_*`) classifies every street it
//! produces as `Local`, `Pedestrian`, or (its own "v0.6" addition) real
//! `Arterial` streets -- see `p49_looped_local_roads.rs` for the
//! Local/Pedestrian distinction this opinion's sibling already relies on.
//! This check is deliberately scoped to Pedestrian/Local crossings only:
//! for every real intersection (streets sharing a coordinate-snapped
//! endpoint, same node-finding approach as `p49`/`p50`) where a
//! `Pedestrian`-classified street meets a `Local`-classified one, this
//! checks the crossing angle against 90 degrees -- directly testing
//! "cross roads perpendicularly," not "run alongside them." Arterial
//! crossings aren't part of this claim (Alexander's own text is about
//! pedestrian paths threading a local road network, not the arterial
//! backbone itself) and aren't counted here.
//!
//! `value` = mean of `(1 - |angle - 90| / 90)` across every real
//! pedestrian/local crossing, clamped to `[0, 1]`. `NoView` if the
//! network has no real Pedestrian/Local crossing at all (e.g. an
//! all-Local network, or `path_network` hasn't run).

use std::collections::BTreeMap;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P52NetworkOfPathsAndCars;

fn node_key(p: &LngLat) -> (i64, i64) {
    ((p.lng * 1e7).round() as i64, (p.lat * 1e7).round() as i64)
}

fn angle_deg(a: (f64, f64), b: (f64, f64)) -> f64 {
    let dot = a.0 * b.0 + a.1 * b.1;
    let mag = ((a.0 * a.0 + a.1 * a.1).sqrt()) * ((b.0 * b.0 + b.1 * b.1).sqrt());
    if mag <= 0.0 {
        return 0.0;
    }
    (dot / mag).clamp(-1.0, 1.0).acos().to_degrees()
}

impl Opinion for P52NetworkOfPathsAndCars {
    fn name(&self) -> &'static str {
        "p52_network_of_paths_and_cars"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p52".into(),
            display: "Alexander et al., A Pattern Language, Pattern 52 (Network of Paths and Cars)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Network-of-Paths-and-Cars-(52)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        // Per node: outward direction vectors, tagged by which street's
        // classification they came from.
        let mut node_vectors: std::collections::HashMap<(i64, i64), Vec<((f64, f64), Option<String>)>> =
            std::collections::HashMap::new();
        for s in &n.streets {
            if s.centerline.len() < 2 {
                continue;
            }
            let a = s.centerline.first().unwrap();
            let b = s.centerline.last().unwrap();
            node_vectors.entry(node_key(a)).or_default().push(((b.lng - a.lng, b.lat - a.lat), s.classification.clone()));
            node_vectors.entry(node_key(b)).or_default().push(((a.lng - b.lng, a.lat - b.lat), s.classification.clone()));
        }

        let mut crossing_scores: Vec<f64> = Vec::new();
        for vectors in node_vectors.values() {
            let pedestrian: Vec<&(f64, f64)> = vectors.iter()
                .filter(|(_, c)| c.as_deref() == Some("pedestrian"))
                .map(|(v, _)| v)
                .collect();
            let local: Vec<&(f64, f64)> = vectors.iter()
                .filter(|(_, c)| c.as_deref() == Some("local"))
                .map(|(v, _)| v)
                .collect();
            for p in &pedestrian {
                for l in &local {
                    let angle = angle_deg(**p, **l);
                    crossing_scores.push((1.0 - (angle - 90.0).abs() / 90.0).clamp(0.0, 1.0));
                }
            }
        }

        if crossing_scores.is_empty() {
            return OpinionOutput::NoView {
                reason: "No real Pedestrian/Local street crossing in this neighborhood -- run PathNetwork first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let value = crossing_scores.iter().sum::<f64>() / crossing_scores.len() as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("perpendicularity".into(), value);

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_pedestrian_local_crossings".into(), crossing_scores.len().to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} real Pedestrian/Local street crossing(s); mean perpendicularity score {:.2} \
                 (1.0 = a clean 90-degree crossing).",
                crossing_scores.len(), value
            ),
            sub_scores,
            details,
            caveats: vec![
                "Node identity is coordinate-snapping (1cm precision) on real street endpoints, \
                 not general line-crossing detection -- a Pedestrian path that geometrically \
                 crosses a Local road without sharing an endpoint vertex (a real grade separation, \
                 or two streets that just happen to overlap) isn't counted as a crossing at all.".into(),
                "Only checks the ANGLE at real crossings, not Alexander's other prescription -- a \
                 separate, mostly-freestanding path network reached 'one path at a time through the \
                 center of blocks' -- since this pipeline generates one street network per site, not \
                 an incrementally-added second layer.".into(),
                "Angles are computed directly on raw lng/lat deltas with no cos(latitude) \
                 correction, same as p50_t_junctions.rs -- a real but usually small distortion at \
                 neighborhood scale.".into(),
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
                layer_provenance: Default::default(), label: "P52 unit fixture".into(),
            },
        }
    }

    fn street(id: &str, a: (f64, f64), b: (f64, f64), classification: &str) -> Street {
        let m = 1.0 / 111_320.0;
        Street {
            id: id.into(),
            centerline: vec![LngLat::new(a.0 * m, a.1 * m), LngLat::new(b.0 * m, b.1 * m)],
            classification: Some(classification.into()),
            row_width_m: Some(5.5), surface: None,
        }
    }

    #[test]
    fn no_streets_is_no_view() {
        assert!(matches!(P52NetworkOfPathsAndCars.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn all_local_network_is_no_view() {
        // No Pedestrian streets at all -- nothing to check a crossing angle against.
        let n = nbhd(vec![street("S1", (0.0, 0.0), (100.0, 0.0), "local")]);
        assert!(matches!(P52NetworkOfPathsAndCars.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_perpendicular_pedestrian_crossing_scores_full() {
        // Local road along the x-axis (two segments, so an endpoint
        // actually sits AT the origin -- node detection is endpoint
        // snapping, not point-on-line), crossed by a Pedestrian path
        // straight up (+y) from that same origin -- a clean 90-degree
        // crossing.
        let n = nbhd(vec![
            street("LOCAL_A", (-100.0, 0.0), (0.0, 0.0), "local"),
            street("LOCAL_B", (0.0, 0.0), (100.0, 0.0), "local"),
            street("PED", (0.0, 0.0), (0.0, 100.0), "pedestrian"),
        ]);
        let out = P52NetworkOfPathsAndCars.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!(sub_scores["perpendicularity"] > 0.95, "got {}", sub_scores["perpendicularity"]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_parallel_pedestrian_path_scores_zero() {
        // Pedestrian path running alongside (parallel to) the local road
        // -- exactly the "running alongside" case Alexander warns against.
        let n = nbhd(vec![
            street("LOCAL", (0.0, 0.0), (100.0, 0.0), "local"),
            street("PED", (0.0, 0.0), (100.0, 0.0), "pedestrian"),
        ]);
        let out = P52NetworkOfPathsAndCars.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!(sub_scores["perpendicularity"] < 0.05, "got {}", sub_scores["perpendicularity"]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
