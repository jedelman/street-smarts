//! P49 Looped Local Roads — local roads should form real loops (nothing
//! shortcuts through them) and stay narrow, not read as a through-traffic
//! tree.
//!
//! From Alexander, *A Pattern Language*, Pattern 49 (p. 260), via
//! patternlanguage.cc/Patterns/Looped-Local-Roads-(49):
//! > **Problem:** Nobody wants fast through-traffic going by their homes.
//! > **Solution:** Lay out local roads so that they form loops... Do not
//! > allow any one loop to serve more than 50 cars, and keep the road
//! > really narrow -- 17 to 20 feet is quite enough.
//!
//! # A real gap this opinion found, and closed
//!
//! `path_network.rs`'s original v0.2 design built a Minimum Spanning Tree
//! as its `Local`-classified backbone (exactly V-1 edges for V vertices BY
//! DEFINITION -- zero cycles), then added extra edges classified
//! `Pedestrian`, not `Local`, to relieve dead-ends. So every
//! `Local`-classified street it produced was, by construction, part of a
//! pure tree: the graph-cycle check below (`has_loop`) found none among
//! Local-classified streets on any real pipeline output -- not a bug in
//! this opinion, but the generator's own MST-backbone design never
//! looping its LOCAL roads at all. `path_network`'s `path_width_m` also
//! defaulted to 4.0m, below Alexander's literal 17-20ft (5.18-6.10m)
//! range.
//!
//! v0.3 of `path_network` closed both: a new, separate `local_loop_budget`
//! parameter (default 1.0) adds real `Local`-classified loop edges on top
//! of the MST, and `path_width_m`'s default moved to 5.5m. This opinion
//! still checks against Alexander's own numbers, not whatever the
//! generator's current defaults happen to be, so it stays a real check
//! rather than a tautology -- see `path_network.rs`'s own module doc for
//! the fix.
//!
//! # Two sub-scores
//!
//! - `has_loop`: for the graph formed by `Local`-classified streets
//!   (nodes = snapped centerline endpoints, edges = streets), whether each
//!   connected component has `edges >= nodes` (a real cycle) rather than
//!   `edges == nodes - 1` (a pure tree) -- `1.0` if ALL components loop,
//!   `0.0` if none do, proportional between.
//! - `width_in_range`: fraction of `Local`-classified streets whose
//!   `row_width_m` falls in Alexander's literal 17-20 foot range
//!   (5.18-6.10m).
//!
//! `value = 0.5 * has_loop + 0.5 * width_in_range`. Doesn't check the "no
//! more than 50 cars per loop" traffic-volume requirement -- no vehicle-
//! count or dwelling-unit data exists in this schema.

use std::collections::{BTreeMap, HashMap};
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P49LoopedLocalRoads;

const MIN_WIDTH_M: f64 = 5.18; // 17 feet
const MAX_WIDTH_M: f64 = 6.10; // 20 feet

/// Snap a coordinate to ~1cm precision so shared endpoints (the same
/// block centroid reused across several edges) compare equal even with
/// tiny floating-point noise.
fn node_key(lng: f64, lat: f64) -> (i64, i64) {
    ((lng * 1e7).round() as i64, (lat * 1e7).round() as i64)
}

struct UnionFind {
    parent: HashMap<(i64, i64), (i64, i64)>,
}
impl UnionFind {
    fn new() -> Self {
        Self { parent: HashMap::new() }
    }
    fn find(&mut self, x: (i64, i64)) -> (i64, i64) {
        let p = *self.parent.entry(x).or_insert(x);
        if p == x {
            x
        } else {
            let root = self.find(p);
            self.parent.insert(x, root);
            root
        }
    }
    fn union(&mut self, a: (i64, i64), b: (i64, i64)) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra != rb {
            self.parent.insert(ra, rb);
        }
    }
}

impl Opinion for P49LoopedLocalRoads {
    fn name(&self) -> &'static str {
        "p49_looped_local_roads"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p49".into(),
            display: "Alexander et al., A Pattern Language, Pattern 49 (Looped Local Roads)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Looped-Local-Roads-(49)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let local_streets: Vec<_> = n.streets.iter()
            .filter(|s| s.classification.as_deref() == Some("local") && s.centerline.len() >= 2)
            .collect();
        if local_streets.is_empty() {
            return OpinionOutput::NoView {
                reason: "No Local-classified streets in this neighborhood -- run PathNetwork first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut uf = UnionFind::new();
        let mut nodes: std::collections::HashSet<(i64, i64)> = std::collections::HashSet::new();
        for s in &local_streets {
            let a = s.centerline.first().unwrap();
            let b = s.centerline.last().unwrap();
            let ka = node_key(a.lng, a.lat);
            let kb = node_key(b.lng, b.lat);
            nodes.insert(ka);
            nodes.insert(kb);
            uf.union(ka, kb);
        }

        let mut component_nodes: HashMap<(i64, i64), usize> = HashMap::new();
        for &k in &nodes {
            *component_nodes.entry(uf.find(k)).or_insert(0) += 1;
        }
        let mut component_edges: HashMap<(i64, i64), usize> = HashMap::new();
        for s in &local_streets {
            let a = s.centerline.first().unwrap();
            let root = uf.find(node_key(a.lng, a.lat));
            *component_edges.entry(root).or_insert(0) += 1;
        }

        let n_components = component_nodes.len();
        let n_looping = component_nodes.iter()
            .filter(|(root, &v)| *component_edges.get(*root).unwrap_or(&0) >= v)
            .count();
        let has_loop = n_looping as f64 / n_components as f64;

        let in_range = local_streets.iter()
            .filter(|s| s.row_width_m.map(|w| (MIN_WIDTH_M..=MAX_WIDTH_M).contains(&w)).unwrap_or(false))
            .count();
        let width_in_range = in_range as f64 / local_streets.len() as f64;

        let value = 0.5 * has_loop + 0.5 * width_in_range;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("has_loop".into(), has_loop);
        sub_scores.insert("width_in_range".into(), width_in_range);

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_local_streets".into(), local_streets.len().to_string());
        details.insert("n_connected_components".into(), n_components.to_string());
        details.insert("n_components_with_a_loop".into(), n_looping.to_string());
        details.insert("target_width_range_m".into(), format!("{MIN_WIDTH_M:.2}-{MAX_WIDTH_M:.2}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} Local-classified street(s) across {} connected component(s); {}/{} component(s) \
                 contain a real loop; {:.0}% of streets fall in Alexander's 17-20ft width range.",
                local_streets.len(), n_components, n_looping, n_components, width_in_range * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Cycle detection is a real graph-theoretic check (edges >= nodes per connected \
                 component), not a visual/topological loop-legibility check -- a genuinely tangled \
                 but cycle-containing network scores the same as a clean, legible loop.".into(),
                "Doesn't check the '50 cars per loop' traffic-volume cap -- no vehicle-count or \
                 dwelling-unit data exists in this schema.".into(),
                "Node identity is coordinate-snapping (1cm precision), not real intersection \
                 detection -- two streets that cross geometrically without sharing an endpoint \
                 vertex aren't counted as connected.".into(),
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
    use street_smarts_core::geometry::LngLat;
    use street_smarts_core::nir::{NeighborhoodMeta, Street};

    fn nbhd(streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings: vec![], streets, open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P49 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn street(id: &str, a: (f64, f64), b: (f64, f64), width: f64) -> Street {
        let m = 1.0 / 111_320.0;
        Street {
            id: id.into(),
            centerline: vec![LngLat::new(a.0 * m, a.1 * m), LngLat::new(b.0 * m, b.1 * m)],
            classification: Some("local".into()),
            row_width_m: Some(width), surface: None,
        }
    }

    #[test]
    fn no_local_streets_is_no_view() {
        assert!(matches!(P49LoopedLocalRoads.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_tree_has_zero_loops() {
        // 3 nodes, 2 edges -- a pure tree (path A-B-C), no cycle.
        let n = nbhd(vec![
            street("S1", (0.0, 0.0), (100.0, 0.0), 4.0),
            street("S2", (100.0, 0.0), (200.0, 0.0), 4.0),
        ]);
        let out = P49LoopedLocalRoads.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["has_loop"] - 0.0).abs() < 1e-6, "a tree should have zero loops, got {}", sub_scores["has_loop"]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_real_triangle_has_a_loop() {
        // 3 nodes, 3 edges forming a closed triangle -- a real cycle.
        let n = nbhd(vec![
            street("S1", (0.0, 0.0), (100.0, 0.0), 4.0),
            street("S2", (100.0, 0.0), (50.0, 87.0), 4.0),
            street("S3", (50.0, 87.0), (0.0, 0.0), 4.0),
        ]);
        let out = P49LoopedLocalRoads.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["has_loop"] - 1.0).abs() < 1e-6, "a closed triangle should have a loop, got {}", sub_scores["has_loop"]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn width_matching_alexanders_17_to_20_feet_scores_full_width() {
        let n = nbhd(vec![street("S1", (0.0, 0.0), (100.0, 0.0), 5.5)]);
        let out = P49LoopedLocalRoads.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["width_in_range"] - 1.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_4m_width_below_alexanders_range_fails_the_width_check() {
        // Matches path_network's original path_width_m default (4.0)
        // before it was raised to close this exact gap -- kept as a
        // boundary-case regression, see path_network.rs's own module doc.
        let n = nbhd(vec![street("S1", (0.0, 0.0), (100.0, 0.0), 4.0)]);
        let out = P49LoopedLocalRoads.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["width_in_range"] - 0.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn the_generators_own_5_5m_default_clears_the_width_check() {
        // Matches path_network's current path_width_m default (5.5),
        // raised specifically to clear Alexander's literal 17-20ft range.
        let n = nbhd(vec![street("S1", (0.0, 0.0), (100.0, 0.0), 5.5)]);
        let out = P49LoopedLocalRoads.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["width_in_range"] - 1.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_network_with_mst_plus_one_local_loop_edge_has_a_real_loop() {
        // Models path_network's real v0.3 output shape: an MST backbone
        // (A-B, B-C) plus one local_loop_budget edge (C-A) closing the
        // triangle -- all classified 'local', matching real generator
        // output at its current defaults.
        let n = nbhd(vec![
            street("path_A_to_B", (0.0, 0.0), (100.0, 0.0), 5.5),
            street("path_B_to_C", (100.0, 0.0), (50.0, 87.0), 5.5),
            street("localloop_C_to_A", (50.0, 87.0), (0.0, 0.0), 5.5),
        ]);
        let out = P49LoopedLocalRoads.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["has_loop"] - 1.0).abs() < 1e-6, "MST + local_loop_budget edge should close a real loop, got {}", sub_scores["has_loop"]);
                assert!((sub_scores["width_in_range"] - 1.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
