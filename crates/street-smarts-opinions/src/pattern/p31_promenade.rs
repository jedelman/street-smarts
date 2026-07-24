//! P31 Promenade — a community needs a central, linear public-life spine
//! linking its main activity nodes, within a 10-minute walk of every
//! point.
//!
//! From Alexander, *A Pattern Language*, Pattern 31 (p. 168), via
//! patternlanguage.cc/Patterns/Promenade-(31):
//! > **Problem:** Each subculture needs a center for its public life: a
//! > place where you can go to see people, and to be seen.
//! > **Solution:** Encourage the gradual formation of a promenade at the
//! > heart of every community, linking the main activity nodes, and
//! > placed centrally, so that each point in the community is within 10
//! > minutes' walk of it. Put main points of attraction at the two ends,
//! > to keep a constant movement up and down.
//!
//! # v0.2: a real connectivity check, now that ActivityNode exists for real
//!
//! `Neighborhood.activity_nodes` is no longer permanently empty: since
//! this session's `p61_small_public_squares` "v0.7" update, every square
//! that operator places gets one real `ActivityNode`. There is still no
//! "promenade" path type distinct from `Street.classification`'s four
//! values -- Alexander's own literal path type isn't in this schema and
//! isn't being invented here. What IS real and checkable: whether real
//! streets actually LINK the real activity nodes into one connected
//! spine, the way a promenade would, rather than leaving them scattered
//! and disconnected.
//!
//! For each pair of real `ActivityNode`s, a real street "links" them if
//! it has one endpoint within `LINK_THRESHOLD_M` (20m) of one node and
//! its other endpoint within that same threshold of the other. This
//! builds a real graph (nodes = ActivityNodes, edges = real direct street
//! links) and scores its connectivity: `(largest connected component size
//! - 1) / (total nodes - 1)` -- `1.0` when every real activity node
//! chains into one connected spine (exactly what "linking the main
//! activity nodes" describes), `0.0` when every node sits isolated with
//! no real link to any other.
//!
//! **Honestly scoped.** This checks CONNECTIVITY, not Alexander's own
//! "placed centrally... within 10 minutes' walk of every point" or "main
//! points of attraction at the two ends" -- no walk-time-from-every-point
//! computation or attraction-ranking exists in this schema. A real,
//! narrower proxy: does a real path exist linking the nodes at all, not
//! whether that path is well-placed or well-anchored. Also worth naming:
//! `p61_small_public_squares` only ever links squares it places within
//! the SAME per-block call (its own MST backbone, built once per block);
//! activity nodes on different blocks currently have no real street
//! connecting them directly, so a low score here on a multi-block site
//! is an honest structural fact about this pipeline's own per-block
//! square placement, not a bug in this check.

use std::collections::BTreeMap;
use street_smarts_core::geometry::{haversine_m, LngLat};
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P31Promenade;

const LINK_THRESHOLD_M: f64 = 20.0;

/// Union-Find over activity-node indices, for real connected-component
/// sizing -- no external crate needed for a graph this small.
fn find(parent: &mut [usize], x: usize) -> usize {
    if parent[x] != x {
        parent[x] = find(parent, parent[x]);
    }
    parent[x]
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let (ra, rb) = (find(parent, a), find(parent, b));
    if ra != rb {
        parent[ra] = rb;
    }
}

impl Opinion for P31Promenade {
    fn name(&self) -> &'static str {
        "p31_promenade"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p31".into(),
            display: "Alexander et al., A Pattern Language, Pattern 31 (Promenade)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Promenade-(31)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        if n.activity_nodes.len() < 2 {
            return OpinionOutput::NoView {
                reason: format!(
                    "{} real ActivityNode(s) -- a promenade needs at least two to link. Run \
                     p61_small_public_squares first.",
                    n.activity_nodes.len()
                ),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let locations: Vec<LngLat> = n.activity_nodes.iter().map(|a| a.location).collect();
        let n_nodes = locations.len();
        let mut parent: Vec<usize> = (0..n_nodes).collect();
        let mut n_links = 0;

        for s in &n.streets {
            if s.centerline.len() < 2 {
                continue;
            }
            let a = s.centerline.first().unwrap();
            let b = s.centerline.last().unwrap();
            // Which node (if any) is each real endpoint near?
            let near_a = locations.iter().position(|loc| haversine_m(a, loc) <= LINK_THRESHOLD_M);
            let near_b = locations.iter().position(|loc| haversine_m(b, loc) <= LINK_THRESHOLD_M);
            if let (Some(i), Some(j)) = (near_a, near_b) {
                if i != j {
                    union(&mut parent, i, j);
                    n_links += 1;
                }
            }
        }

        let mut component_sizes: BTreeMap<usize, usize> = BTreeMap::new();
        for i in 0..n_nodes {
            let root = find(&mut parent, i);
            *component_sizes.entry(root).or_insert(0) += 1;
        }
        let largest_component = component_sizes.values().copied().max().unwrap_or(1);

        let value = (largest_component as f64 - 1.0) / (n_nodes as f64 - 1.0);

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("connectivity_fraction".into(), value);

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_activity_nodes".into(), n_nodes.to_string());
        details.insert("n_direct_links".into(), n_links.to_string());
        details.insert("largest_connected_spine".into(), largest_component.to_string());
        details.insert("link_threshold_m".into(), format!("{LINK_THRESHOLD_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} real ActivityNode(s); largest real street-linked spine covers {} of them \
                 ({:.0}% connectivity).",
                n_nodes, largest_component, value * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Checks real street connectivity between ActivityNodes only -- does not check \
                 Alexander's own 'placed centrally, within 10 minutes' walk of every point' or \
                 'main points of attraction at the two ends' (no walk-time or attraction-ranking \
                 data exists in this schema).".into(),
                "p61_small_public_squares only links squares placed within the same per-block \
                 call (its own MST backbone) -- activity nodes on different blocks currently have \
                 no real street connecting them directly, so a low score on a multi-block site is \
                 a real structural fact about this pipeline's own per-block placement, not a bug \
                 in this check. See this opinion's own 'v0.2' module doc.".into(),
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
    use street_smarts_core::nir::{ActivityKind, ActivityNode, NeighborhoodMeta, Street};

    fn nbhd(activity_nodes: Vec<ActivityNode>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings: vec![], streets, open_space: vec![],
            boundaries: vec![], activity_nodes,
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P31 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn node_at(id: &str, x_m: f64, y_m: f64) -> ActivityNode {
        let m = 1.0 / 111_320.0;
        ActivityNode {
            id: id.into(),
            location: LngLat::new(x_m * m, y_m * m),
            kind: ActivityKind::Civic,
            intensity: None,
            label: None,
            activity_fit: Default::default(),
            publicness: None,
        }
    }

    fn street_between(id: &str, a: (f64, f64), b: (f64, f64)) -> Street {
        let m = 1.0 / 111_320.0;
        Street {
            id: id.into(),
            centerline: vec![LngLat::new(a.0 * m, a.1 * m), LngLat::new(b.0 * m, b.1 * m)],
            classification: Some("pedestrian".into()),
            row_width_m: Some(3.0),
            surface: None,
        }
    }

    #[test]
    fn fewer_than_two_nodes_is_no_view() {
        assert!(matches!(P31Promenade.evaluate(&nbhd(vec![node_at("A", 0.0, 0.0)], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn three_nodes_chained_by_real_streets_score_full_connectivity() {
        let nodes = vec![node_at("A", 0.0, 0.0), node_at("B", 50.0, 0.0), node_at("C", 100.0, 0.0)];
        let streets = vec![
            street_between("S1", (0.0, 0.0), (50.0, 0.0)),
            street_between("S2", (50.0, 0.0), (100.0, 0.0)),
        ];
        let out = P31Promenade.evaluate(&nbhd(nodes, streets));
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn disconnected_nodes_score_zero_connectivity() {
        let nodes = vec![node_at("A", 0.0, 0.0), node_at("B", 500.0, 0.0), node_at("C", 1000.0, 0.0)];
        let out = P31Promenade.evaluate(&nbhd(nodes, vec![]));
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 0.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_partial_chain_scores_partial_credit() {
        // A-B linked, C isolated -- largest component 2 of 3 nodes.
        let nodes = vec![node_at("A", 0.0, 0.0), node_at("B", 50.0, 0.0), node_at("C", 1000.0, 0.0)];
        let streets = vec![street_between("S1", (0.0, 0.0), (50.0, 0.0))];
        let out = P31Promenade.evaluate(&nbhd(nodes, streets));
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 0.5).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
