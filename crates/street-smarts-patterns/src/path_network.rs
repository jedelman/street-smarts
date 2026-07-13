//! Path network — lays paths/streets between blocks.
//!
//! Pulls from:
//! - **P52 Network of Paths and Cars** (the path system is the spine)
//! - **P98 Circulation Realms** (distinct walking vs driving zones)
//! - **P120 Paths and Goals** (paths go where people actually want to walk)
//!
//! # v0.2: MST + loop budget, replacing threshold-adjacency
//!
//! The original v0.1 connected every pair of blocks within a distance
//! threshold (median inter-block distance x a multiplier). That produces a
//! dense proximity mesh -- structurally the OPPOSITE of what P52 actually
//! prescribes. Alexander's own text: cars should be kept to a limited,
//! sparse network, not given a fully-connected grid; but a pure branching
//! tree creates dead-ends, so the network should have a FEW loops, not many.
//!
//! This version builds a Minimum Spanning Tree (Kruskal's algorithm, plain
//! union-find, no external crate) over block centroids as the connectivity
//! backbone -- the fewest possible edges that still reach every block, which
//! is a direct, honest reading of "kept to a limited network." It then adds
//! back the `loop_budget` cheapest edges NOT already in the MST, to relieve
//! dead-ends without reverting to mesh density.
//!
//! MST edges are classified `"local"` (the guaranteed-connectivity backbone
//! -- the real reading of "the car network"). Loop-budget edges are
//! classified `"pedestrian"` (supplementary shortcuts that reduce
//! backtracking on foot). This is a real topological distinction, not the
//! old `classification_mode`/`j%2==0` parity hack it replaces.
//!
//! What it does NOT do yet:
//! - Connect to the existing street grid (Princess Anne Rd, etc.) — needs
//!   knowledge of which boundary parcels touch existing streets
//! - Route paths around obstacles (currently they're straight segments
//!   between block centroids) — MST/loop topology is real, but each edge
//!   is still a straight line, not an obstacle-aware route
//! - Aggregate adjacent paths into named streets (each segment is its own
//!   `Street` entity with an auto-id)

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{average_centroid, lnglat_to_local, Pt2};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Neighborhood, Parcel, Street};
use street_smarts_core::opinion::SourceCitation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathNetworkParams {
    /// How many extra edges to add beyond the MST backbone, to relieve
    /// dead-ends with a few loops. 0 = pure tree (real dead-ends exist).
    /// Each unit adds exactly one more edge, taken in ascending distance
    /// order from whatever the MST didn't already use.
    pub loop_budget: f64,
    /// Right-of-way width in metres for each path segment.
    pub path_width_m: f64,
}

impl Parameters for PathNetworkParams {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "loop_budget",
                "Extra edges beyond the MST backbone, to relieve dead-ends without reverting to mesh density.",
                0.0, 10.0, 2.0,
            ),
            ParamSpec::float(
                "path_width_m",
                "Right-of-way width per path segment.",
                2.0, 12.0, 4.0,
            ).with_unit("m"),
        ]
    }
    fn defaults() -> Self {
        Self {
            loop_budget: 2.0,
            path_width_m: 4.0,
        }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.loop_budget, self.path_width_m]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.loop_budget = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.path_width_m = s.clamp(*x); }
        p
    }
}

/// Plain union-find (path compression + union by size). No external crate --
/// this is a ~15-line structure, doesn't warrant a dependency.
struct UnionFind {
    parent: Vec<usize>,
    size: Vec<usize>,
}
impl UnionFind {
    fn new(n: usize) -> Self {
        Self { parent: (0..n).collect(), size: vec![1; n] }
    }
    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }
    fn union(&mut self, a: usize, b: usize) -> bool {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb { return false; }
        let (big, small) = if self.size[ra] >= self.size[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        true
    }
}

pub struct PathNetwork;

impl PatternOperator for PathNetwork {
    type Params = PathNetworkParams;

    fn name(&self) -> &'static str { "path_network" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p52_p98_p120".into(),
            display: "Alexander et al., APL — Patterns 52/98/120 (Network of Paths, Circulation Realms, Paths and Goals)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl52/apl52.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Lay paths between adjacent blocks (path graph, v0.1)."
    }

    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        _seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("path_network requires parcel_id='*' (operates on the block graph)".into());
        }

        // Group pads by BLOCK_n.
        let mut blocks: HashMap<String, Vec<&Parcel>> = HashMap::new();
        for p in &nbhd.parcels {
            if let Some(spec) = &p.spec {
                if spec.starts_with("BLOCK_") {
                    blocks.entry(spec.clone()).or_default().push(p);
                }
            }
        }
        if blocks.len() < 2 {
            return Err(format!(
                "path_network: need at least 2 blocks (found {}). Run block_grouping first.",
                blocks.len()
            ));
        }

        // Anchor projection at the mean of all block centroids.
        let mut all_lng = 0.0;
        let mut all_lat = 0.0;
        let mut n = 0;
        for parcels in blocks.values() {
            for p in parcels {
                let c = average_centroid(&p.polygon.outer);
                all_lng += c.lng;
                all_lat += c.lat;
                n += 1;
            }
        }
        let origin = LngLat::new(all_lng / n as f64, all_lat / n as f64);

        // Compute each block's centroid in local meters AND lng/lat.
        let mut block_ids: Vec<String> = blocks.keys().cloned().collect();
        block_ids.sort(); // deterministic order
        let mut centers_local: Vec<Pt2> = Vec::with_capacity(block_ids.len());
        let mut centers_wgs: Vec<LngLat> = Vec::with_capacity(block_ids.len());
        for bid in &block_ids {
            let parcels = &blocks[bid];
            let mut lng = 0.0;
            let mut lat = 0.0;
            for p in parcels {
                let c = average_centroid(&p.polygon.outer);
                lng += c.lng;
                lat += c.lat;
            }
            let avg = LngLat::new(lng / parcels.len() as f64, lat / parcels.len() as f64);
            centers_local.push(lnglat_to_local(&avg, &origin));
            centers_wgs.push(avg);
        }

        // Compute all pairwise distances, sorted ascending -- Kruskal's needs this order.
        let nb = centers_local.len();
        let mut edges: Vec<(usize, usize, f64)> = Vec::new();
        for i in 0..nb {
            for j in (i + 1)..nb {
                edges.push((i, j, centers_local[i].dist(centers_local[j])));
            }
        }
        edges.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

        // MST backbone (Kruskal's): the fewest edges that connect every
        // block. This IS the honest reading of "cars kept to a limited
        // network" -- not a tuned distance threshold.
        let mut uf = UnionFind::new(nb);
        let mut mst_edges: Vec<(usize, usize, f64)> = Vec::new();
        let mut remaining: Vec<(usize, usize, f64)> = Vec::new();
        for &(i, j, d) in &edges {
            if uf.union(i, j) {
                mst_edges.push((i, j, d));
            } else {
                remaining.push((i, j, d));
            }
        }

        if mst_edges.len() < nb.saturating_sub(1) {
            return Err(format!(
                "path_network: block centroid graph is disconnected ({} MST edges for {} blocks, expected {}). \
                 This shouldn't happen for a complete graph -- investigate degenerate/duplicate centroids.",
                mst_edges.len(), nb, nb.saturating_sub(1)
            ));
        }

        // Loop budget: cheapest edges NOT already in the MST, to relieve
        // dead-ends without reverting to full mesh density. `remaining` is
        // already sorted ascending (built from the sorted `edges` pass).
        let loop_budget = params.loop_budget.round().max(0.0) as usize;
        let loop_edges: Vec<(usize, usize, f64)> =
            remaining.into_iter().take(loop_budget).collect();

        // Emit Street segments: MST = backbone ("local"), loop edges =
        // supplementary shortcuts ("pedestrian"). Real topological
        // distinction, not a parity hack.
        let mut streets: Vec<Street> = Vec::new();
        for &(i, j, _d) in mst_edges.iter() {
            streets.push(Street {
                id: format!("path_{}_to_{}", block_ids[i], block_ids[j]),
                centerline: vec![centers_wgs[i], centers_wgs[j]],
                classification: Some("local".into()),
                row_width_m: Some(params.path_width_m),
            });
        }
        for &(i, j, _d) in loop_edges.iter() {
            streets.push(Street {
                id: format!("loop_{}_to_{}", block_ids[i], block_ids[j]),
                centerline: vec![centers_wgs[i], centers_wgs[j]],
                classification: Some("pedestrian".into()),
                row_width_m: Some(params.path_width_m),
            });
        }

        if streets.is_empty() {
            return Err("path_network: produced zero edges -- should be unreachable for nb>=2.".into());
        }

        let mut steps = vec![
            format!(
                "{} blocks -> MST backbone: {} edges (guaranteed connectivity, classified 'local')",
                nb, mst_edges.len()
            ),
        ];
        steps.push(format!(
            "loop_budget={} -> {} extra edges added (classified 'pedestrian')",
            loop_budget, loop_edges.len()
        ));
        steps.push(format!("Emitted {} Street segments at {}m right-of-way", streets.len(), params.path_width_m as u32));

        let trace = SubdivisionTrace {
            operator_name: "path_network".into(),
            operator_source: self.source(),
            headline: format!(
                "Laid {} MST backbone + {} loop edge(s) across {} blocks.",
                mst_edges.len(), loop_edges.len(), nb
            ),
            steps,
            caveats: vec![
                "Paths are straight segments between block centroids, even along the MST/loop \
                 backbone -- edges are topologically real (Kruskal's MST + loop budget), but each \
                 individual edge does not yet route around obstacles or follow shared boundaries.".into(),
                "Local/pedestrian classification reflects real topology (MST backbone vs. \
                 supplementary loop edge), but is still a placeholder for real P98 Circulation \
                 Realms reasoning -- it doesn't know which blocks actually need vehicular access.".into(),
                "Does not connect to the existing street grid (Princess Anne Rd, etc.) -- needs \
                 knowledge of which boundary parcels touch existing streets.".into(),
                "Adjacent paths are not aggregated into named streets. Each segment is its own \
                 NIR Street entity.".into(),
            ],
            seed: 0,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels: vec![],
            new_open_space: vec![],
            new_buildings: vec![],
            new_streets: streets,
            replaced_parcel_ids: vec![],
            trace,
        })
    }
}
