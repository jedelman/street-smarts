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
//! This version builds a Minimum Spanning Tree (Kruskal's algorithm, via
//! `planar::kruskal_mst` -- shared with P61, which needs the same "fewest
//! edges that still connect everything" reasoning to link its small
//! squares) over block centroids as the connectivity backbone -- the fewest
//! possible edges that still reach every block, which is a direct, honest
//! reading of "kept to a limited network." It then adds
//! back the `loop_budget` cheapest edges NOT already in the MST, to relieve
//! dead-ends without reverting to mesh density.
//!
//! MST edges are classified `"local"` (the guaranteed-connectivity backbone
//! -- the real reading of "the car network"). Loop-budget edges are
//! classified `"pedestrian"` (supplementary shortcuts that reduce
//! backtracking on foot). This is a real topological distinction, not the
//! old `classification_mode`/`j%2==0` parity hack it replaces.
//!
//! # v0.3: `local_loop_budget`, closing P49's real gap
//!
//! v0.2's MST backbone has exactly V-1 edges for V blocks by definition --
//! zero cycles, a pure tree, no matter how large `loop_budget` is (those
//! edges are all classified Pedestrian, not Local). `p49_looped_local_roads`
//! found this: Alexander's P49 is specifically about the LOCAL (car) road
//! network forming loops ("lay out local roads so that they form loops"),
//! which the old design never produced. `local_loop_budget` adds a second,
//! separate small budget of extra edges -- taken from whatever
//! `loop_budget`'s share of `remaining` didn't already use -- classified
//! `Local` instead of `Pedestrian`. Deliberately additive rather than
//! reclassifying `loop_budget`'s own edges, so existing Pedestrian-shortcut
//! behavior (and everything downstream that reads it) is unchanged.
//! `path_width_m`'s default also moved from 4.0m to 5.5m, into Alexander's
//! literal 17-20 foot (5.18-6.10m) range for local roads -- the other real
//! gap `p49_looped_local_roads` found.
//!
//! # v0.4: degree-capped loop-edge selection, closing P50's real gap
//!
//! `p50_t_junctions` checks that real intersections (nodes where 3+ streets
//! share a coordinate-snapped endpoint) meet as clean three-way T's, not
//! four-way-or-more crossings ("avoid four-way intersections and crossing
//! movements" -- Alexander's own text). The old `loop_budget`/
//! `local_loop_budget` selection took the cheapest remaining edges with no
//! regard for how many edges already met at each endpoint, so a loop edge
//! could freely push an already three-way node (MST degree 2, or one prior
//! loop edge) to a four-way crossing. `select_loop_edges` now tracks each
//! block's running degree (starting from the MST backbone) and skips any
//! candidate edge that would push either endpoint to degree 4+, before
//! falling through to the next-cheapest candidate -- a real, enforced
//! topology constraint on this generator's own existing output, not a
//! guess. This directly targets P50's `three_way_fraction` sub-score. It
//! does NOT touch `near_90_fraction` -- the angle at which edges meet is a
//! function of block position (computed upstream by `block_grouping`), not
//! something this edge-selection pass controls; a real fix for that would
//! mean moving or re-routing block centroids, out of scope here.
//! Skipped-for-degree candidates are counted and reported in the trace.
//!
//! # v0.5: real site-perimeter Boundary, closing P53's real gap
//!
//! `Neighborhood.boundaries` is a real, typed field -- before this, no
//! operator anywhere in this pipeline ever populated it (the same class
//! of real-field-no-producer gap `p61_small_public_squares` closed for
//! `activity_nodes`). This operator already runs site-scale over every
//! `BLOCK_n` parcel, so it now also computes the real convex hull of
//! every block's own outer-ring vertices and emits it as one
//! `Boundary { kind: Jurisdictional }` -- a real, computable site
//! perimeter, not a fabricated one. Real, honest limitation: a convex
//! hull is exact for a convex site and an over-approximation for a
//! concave one (see this operator's own trace caveat).
//!
//! What it does NOT do yet:
//! - Connect to the existing street grid (Princess Anne Rd, etc.) — needs
//!   knowledge of which boundary parcels touch existing streets
//! - Route paths around obstacles (currently they're straight segments
//!   between block centroids) — MST/loop topology is real, but each edge
//!   is still a straight line, not an obstacle-aware route
//! - Aggregate adjacent paths into named streets (each segment is its own
//!   `Street` entity with an auto-id)
//!
//! # v0.6: Arterial classification, closing P36/P59/P68's real gap
//!
//! `StreetClassification::Arterial` existed in the enum from the start, but
//! no operator anywhere ever produced it -- confirmed by grep, and
//! documented as a load-bearing "honest gap" in P36 Degrees of Publicness,
//! P59 Quiet Backs, and P68 Connected Play's own doc comments. Those three
//! opinions all correctly check for an Arterial street at runtime; they
//! just never saw one. This operator now reclassifies the `arterial_count`
//! LONGEST edges of its own MST backbone (by real physical length, already
//! computed by Kruskal's) as `Arterial`, at a wider `arterial_width_m`
//! right-of-way and `"asphalt"` surface instead of `"grass_pavers"`. "The
//! longest backbone edge" is a real, deterministic, measurable proxy for
//! "the main through-route" -- not a fabricated classification -- but it is
//! still a proxy: see this operator's own trace caveat for what it doesn't
//! know (actual traffic volume, off-site arterial connections).
//! `arterial_count` defaults to 1.0 (at least one real arterial per site);
//! `arterial_width_m` defaults to 18.0m, a plausible arterial-scale
//! placeholder (wider than any local/pedestrian width this operator
//! produces), not a real code-minimum lookup.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{average_centroid, convex_hull, kruskal_mst, lnglat_to_local, local_to_lnglat, Pt2};
use crate::prng::Prng;
use crate::subdivision::{apply_subdivision, PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use street_smarts_core::components::StreetClassification;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Boundary, BoundaryKind, Neighborhood, Parcel, Street};
use street_smarts_core::opinion::SourceCitation;
use street_smarts_core::world::World;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathNetworkParams {
    /// How many extra edges to add beyond the MST backbone, to relieve
    /// dead-ends with a few loops. 0 = pure tree (real dead-ends exist).
    /// Each unit adds exactly one more edge, taken in ascending distance
    /// order from whatever the MST didn't already use. Classified
    /// `Pedestrian` -- a supplementary shortcut, not part of the car
    /// network. See `local_loop_budget` for closing real loops in the
    /// `Local`-classified backbone itself.
    pub loop_budget: f64,
    /// Same mechanism as `loop_budget` -- extra edges beyond the MST,
    /// taken from whatever `loop_budget`'s share didn't already use -- but
    /// classified `Local`, closing a real loop in the car network itself.
    /// This is what Alexander's P49 Looped Local Roads actually asks for:
    /// "lay out local roads so that they form loops," not pedestrian
    /// shortcuts. Kept as a separate, additive budget from `loop_budget`
    /// rather than reclassifying its edges, so existing `loop_budget`
    /// behavior (and everything that reads Pedestrian-classified streets)
    /// is unchanged.
    pub local_loop_budget: f64,
    /// Right-of-way width in metres for each path segment. Alexander's
    /// own text for local roads: "17 to 20 feet is quite enough"
    /// (5.18-6.10m) -- see P49's own opinion, which checks this.
    pub path_width_m: f64,
    /// How many of the MST backbone's own LONGEST edges (by real physical
    /// length, already computed by Kruskal's) get reclassified `Arterial`
    /// instead of `Local` -- a real, deterministic proxy for "the main
    /// through-route," not an arbitrary pick. 0 = no arterial streets (the
    /// old behavior). See this file's own "v0.6" module doc for why this
    /// exists at all.
    pub arterial_count: f64,
    /// Right-of-way width for an Arterial-classified edge. Wider than
    /// `path_width_m`'s local-road figure -- a plausible arterial-scale
    /// placeholder, not a real code-minimum lookup (same category as
    /// `p95_building_complex`'s `pad_inset_m`).
    pub arterial_width_m: f64,
}

impl Parameters for PathNetworkParams {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "loop_budget",
                "Extra edges beyond the MST backbone, classified Pedestrian, to relieve dead-ends without reverting to mesh density.",
                0.0, 10.0, 2.0,
            ),
            ParamSpec::float(
                "local_loop_budget",
                "Extra edges beyond loop_budget's share, classified Local -- closes real loops in the car network itself (Alexander's P49).",
                0.0, 5.0, 1.0,
            ),
            ParamSpec::float(
                "path_width_m",
                "Right-of-way width per path segment.",
                2.0, 12.0, 5.5,
            ).with_unit("m"),
            ParamSpec::float(
                "arterial_count",
                "How many of the MST backbone's longest edges become Arterial instead of Local.",
                0.0, 3.0, 1.0,
            ),
            ParamSpec::float(
                "arterial_width_m",
                "Right-of-way width for an Arterial-classified edge.",
                12.0, 30.0, 18.0,
            ).with_unit("m"),
        ]
    }
    fn defaults() -> Self {
        Self {
            loop_budget: 2.0,
            local_loop_budget: 1.0,
            path_width_m: 5.5,
            arterial_count: 1.0,
            arterial_width_m: 18.0,
        }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.loop_budget, self.local_loop_budget, self.path_width_m, self.arterial_count, self.arterial_width_m]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.loop_budget = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.local_loop_budget = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) { p.path_width_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(3), v.get(3)) { p.arterial_count = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(4), v.get(4)) { p.arterial_width_m = s.clamp(*x); }
        p
    }
}

/// How far a path's midpoint bulges off the straight line between its
/// endpoints, as a multiple of the path's own `row_width_m` -- always
/// strictly greater than 1.0 so the bulge clears
/// `p121_path_shape`'s own "at least the street's own row_width_m" check
/// by construction, not by chance.
const BULGE_MULTIPLIER: f64 = 1.5;

/// A path is a real place to stay, not just a line to move through
/// (Alexander's P121 Path Shape): bulge the segment's midpoint off the
/// straight line between its endpoints by `row_width_m * BULGE_MULTIPLIER`,
/// perpendicular to the segment, on a side chosen deterministically from
/// `prng` so repeated runs with the same seed reproduce the same shape.
/// Falls back to the plain 2-point straight segment for a degenerate
/// (near-zero-length) edge.
fn bulge_centerline(a_wgs: LngLat, b_wgs: LngLat, origin: &LngLat, row_width_m: f64, prng: &mut Prng) -> Vec<LngLat> {
    let a = lnglat_to_local(&a_wgs, origin);
    let b = lnglat_to_local(&b_wgs, origin);
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= 1e-6 {
        return vec![a_wgs, b_wgs];
    }
    let (px, py) = (-dy / len, dx / len);
    let sign = if prng.range(0.0, 1.0) >= 0.5 { 1.0 } else { -1.0 };
    let bulge_m = row_width_m * BULGE_MULTIPLIER;
    let mid = Pt2::new(
        (a.x + b.x) / 2.0 + px * bulge_m * sign,
        (a.y + b.y) / 2.0 + py * bulge_m * sign,
    );
    vec![a_wgs, local_to_lnglat(mid, origin), b_wgs]
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
        seed: u64,
    ) -> Result<Subdivision, String> {
        self.apply_with_assignments(nbhd, parcel_id, params, seed).map(|(sub, _)| sub)
    }
}

impl PathNetwork {
    /// The real computation, extended to also return each new street's
    /// `StreetClassification` as it's decided -- see `p107_wings_of_light`'s
    /// own `apply_with_assignments` for the same shape and rationale.
    /// `apply()` above is a thin wrapper that discards the extra vec.
    fn apply_with_assignments(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &PathNetworkParams,
        seed: u64,
    ) -> Result<(Subdivision, Vec<(String, StreetClassification)>), String> {
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

        // P53 Main Gateways: the real convex hull of every BLOCK_n parcel's
        // own outer-ring vertices -- a real, computable site perimeter, not
        // a fabricated one. See this file's own "v0.5" module doc.
        let hull_points: Vec<Pt2> = blocks.values()
            .flat_map(|parcels| parcels.iter())
            .flat_map(|p| p.polygon.outer.iter())
            .map(|q| lnglat_to_local(q, &origin))
            .collect();
        let hull_local = convex_hull(&hull_points);
        let site_boundary = if hull_local.len() >= 3 {
            let mut ring_wgs: Vec<LngLat> = hull_local.iter().map(|&p| local_to_lnglat(p, &origin)).collect();
            if let Some(first) = ring_wgs.first().copied() {
                ring_wgs.push(first); // close the ring, same convention Street/Polygon rings use
            }
            Some(Boundary {
                id: "site_perimeter".into(),
                centerline: ring_wgs,
                kind: BoundaryKind::Jurisdictional,
            })
        } else {
            None
        };

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

        // MST backbone (Kruskal's): the fewest edges that connect every
        // block. This IS the honest reading of "cars kept to a limited
        // network" -- not a tuned distance threshold.
        let nb = centers_local.len();
        let crate::planar::MstResult { mst_edges, remaining_edges: remaining } = kruskal_mst(&centers_local);

        if mst_edges.len() < nb.saturating_sub(1) {
            return Err(format!(
                "path_network: block centroid graph is disconnected ({} MST edges for {} blocks, expected {}). \
                 This shouldn't happen for a complete graph -- investigate degenerate/duplicate centroids.",
                mst_edges.len(), nb, nb.saturating_sub(1)
            ));
        }

        // Loop budgets: cheapest edges NOT already in the MST, to relieve
        // dead-ends without reverting to full mesh density. `remaining` is
        // already sorted ascending (built from the sorted `edges` pass).
        // `loop_budget`'s share goes first (Pedestrian shortcuts);
        // `local_loop_budget`'s share comes from whatever's left after
        // that (real loops in the Local/car network -- Alexander's P49).
        // Degree-capped at 3 per node (Alexander's P50 T Junctions -- "avoid
        // four-way intersections"), skipping over any candidate that would
        // push a node past a three-way meeting -- see this file's own "v0.4"
        // module doc.
        let loop_budget = params.loop_budget.round().max(0.0) as usize;
        let local_loop_budget = params.local_loop_budget.round().max(0.0) as usize;
        let mut degree = vec![0usize; nb];
        for &(i, j, _) in mst_edges.iter() {
            degree[i] += 1;
            degree[j] += 1;
        }
        let mut loop_edges: Vec<(usize, usize, f64)> = Vec::new();
        let mut local_loop_edges: Vec<(usize, usize, f64)> = Vec::new();
        let mut skipped_for_four_way = 0usize;
        for &(i, j, d) in remaining.iter() {
            if loop_edges.len() >= loop_budget && local_loop_edges.len() >= local_loop_budget {
                break;
            }
            if degree[i] >= 3 || degree[j] >= 3 {
                skipped_for_four_way += 1;
                continue;
            }
            if loop_edges.len() < loop_budget {
                loop_edges.push((i, j, d));
            } else {
                local_loop_edges.push((i, j, d));
            }
            degree[i] += 1;
            degree[j] += 1;
        }

        // P36/P59/P68's real gap: which MST edges become Arterial. The
        // arterial_count LONGEST MST edges (by real physical length,
        // already computed by Kruskal's) -- a real, deterministic proxy
        // for "the main through-route," not an arbitrary pick. See this
        // file's own "v0.6" module doc.
        let arterial_count = params.arterial_count.round().max(0.0) as usize;
        let mut by_length: Vec<(usize, usize, f64)> = mst_edges.clone();
        by_length.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        let arterial_edges: std::collections::HashSet<(usize, usize)> = by_length.iter()
            .take(arterial_count)
            .map(|&(i, j, _)| (i, j))
            .collect();

        // Emit Street segments: MST = backbone (`Local`, or `Arterial` for
        // its own longest edges), local_loop_edges = real loops in that
        // same backbone (also `Local`), loop_edges = supplementary
        // shortcuts (`Pedestrian`). Real topological distinction, not a
        // parity hack. Each segment's centerline bulges at its midpoint
        // (Alexander's P121 Path Shape) -- drawn from a seeded Prng, in
        // this fixed edge order, so the same seed always reproduces the
        // same shape.
        let mut prng = Prng::new(seed);
        let mut streets: Vec<Street> = Vec::new();
        let mut classification_assignments: Vec<(String, StreetClassification)> = Vec::new();
        let mut n_arterial = 0usize;
        for &(i, j, _d) in mst_edges.iter() {
            let id = format!("path_{}_to_{}", block_ids[i], block_ids[j]);
            let is_arterial = arterial_edges.contains(&(i, j));
            let classification = if is_arterial { StreetClassification::Arterial } else { StreetClassification::Local };
            let width = if is_arterial { params.arterial_width_m } else { params.path_width_m };
            if is_arterial {
                n_arterial += 1;
            }
            streets.push(Street {
                id: id.clone(),
                centerline: bulge_centerline(centers_wgs[i], centers_wgs[j], &origin, width, &mut prng),
                classification: Some(classification.to_label().into()),
                row_width_m: Some(width),
                surface: Some(if is_arterial { "asphalt".into() } else { "grass_pavers".into() }),
            });
            classification_assignments.push((id, classification));
        }
        for &(i, j, _d) in local_loop_edges.iter() {
            let id = format!("localloop_{}_to_{}", block_ids[i], block_ids[j]);
            streets.push(Street {
                id: id.clone(),
                centerline: bulge_centerline(centers_wgs[i], centers_wgs[j], &origin, params.path_width_m, &mut prng),
                classification: Some(StreetClassification::Local.to_label().into()),
                row_width_m: Some(params.path_width_m),
                surface: Some("grass_pavers".into()),
            });
            classification_assignments.push((id, StreetClassification::Local));
        }
        for &(i, j, _d) in loop_edges.iter() {
            let id = format!("loop_{}_to_{}", block_ids[i], block_ids[j]);
            streets.push(Street {
                id: id.clone(),
                centerline: bulge_centerline(centers_wgs[i], centers_wgs[j], &origin, params.path_width_m, &mut prng),
                classification: Some(StreetClassification::Pedestrian.to_label().into()),
                row_width_m: Some(params.path_width_m),
                surface: Some("grass_pavers".into()),
            });
            classification_assignments.push((id, StreetClassification::Pedestrian));
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
            "local_loop_budget={} -> {} extra edges added (classified 'local', closing real car-network loops)",
            local_loop_budget, local_loop_edges.len()
        ));
        steps.push(format!(
            "loop_budget={} -> {} extra edges added (classified 'pedestrian')",
            loop_budget, loop_edges.len()
        ));
        steps.push(format!(
            "{} candidate loop edge(s) skipped -- would have pushed a node past a three-way \
             meeting (Alexander's P50 T Junctions: avoid four-way intersections)",
            skipped_for_four_way
        ));
        steps.push(format!("Emitted {} Street segments at {}m right-of-way", streets.len(), params.path_width_m as u32));
        steps.push(format!(
            "arterial_count={} -> {} of the MST backbone's own longest edges reclassified 'arterial' \
             at {}m right-of-way (real length proxy for the main through-route)",
            arterial_count, n_arterial, params.arterial_width_m as u32
        ));
        steps.push(match &site_boundary {
            Some(b) => format!("Computed a real site-perimeter Boundary ({} vertices, convex hull of every block's own outer ring).", b.centerline.len()),
            None => "Fewer than 3 hull points -- no real site perimeter computed.".into(),
        });

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
                "Loop-edge selection is capped at degree 3 per node (P50 T Junctions), but the MST \
                 backbone itself is not -- a block whose only connection to the rest of the network \
                 requires 4+ MST edges (a genuine hub in the block layout) can still end up a \
                 four-way intersection; this pass only avoids making a discretionary loop edge WORSE, \
                 it doesn't re-route the required backbone.".into(),
                "Does not touch P50's near_90_fraction sub-score -- the angle at which edges meet a \
                 node is a function of block position (set upstream by block_grouping), not \
                 something this edge-selection pass controls.".into(),
                "The site-perimeter Boundary is the convex hull of every block's own outer ring -- \
                 real for a convex or roughly-convex site, but a real concave site's actual edge \
                 would sit inside this hull in places, not exactly on it.".into(),
                "Arterial classification is a proxy: the arterial_count longest MST backbone edges \
                 by real physical length, not a real functional-classification study (traffic \
                 volume, connection to an off-site arterial, etc.). A real site's longest internal \
                 link is a reasonable stand-in for 'the main through-route' but this does not know \
                 whether that edge actually carries through-traffic.".into(),
            ],
            seed: 0,
            params: params.as_map(),
        };

        let sub = Subdivision {
            new_parcels: vec![],
            new_open_space: vec![],
            new_buildings: vec![],
            new_streets: streets,
            new_activity_nodes: vec![],
            new_boundaries: site_boundary.into_iter().collect(),
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: vec![],
            replaced_building_ids: vec![],
            entity_provenance: std::collections::BTreeMap::new(),
            trace,
        };
        Ok((sub, classification_assignments))
    }

    /// Native `System` port (inherent method -- see `system.rs`'s own
    /// module doc). Runs the same computation `apply()` does, then writes
    /// `StreetClassification` directly into the resulting `World` from the
    /// assignments that computation already produced.
    pub fn run_native(&self, world: &World, params: &PathNetworkParams, parcel_id: &str, seed: u64) -> Result<World, String> {
        let nbhd = world.to_neighborhood();
        let (sub, assignments) = self.apply_with_assignments(&nbhd, parcel_id, params, seed)?;
        let new_nbhd = apply_subdivision(&nbhd, &sub);
        let mut new_world = World::from_neighborhood(&new_nbhd);
        for (id, classification) in assignments {
            new_world.street_classifications.insert(id, classification);
        }
        Ok(new_world)
    }
}
