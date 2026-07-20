//! P108 Connected Buildings — merge neighboring building pads that have no
//! real reason to stand apart into one continuous party-wall footprint.
//!
//! From Alexander, *A Pattern Language*, Pattern 108:
//! > Whenever possible, buildings should be connected... Isolated buildings
//! > are a symptom of a world in which community has broken down. Get out
//! > of the habit of "standing back" from buildings and treating them as
//! > objects to be admired; instead, make new buildings which continue the
//! > fabric of buildings that already exists.
//!
//! # Why this exists
//! P95's `pad_inset_m` used to be a real 3.0m setback on every pad, and
//! P107 was applying its OWN setback on top of that (fixed separately --
//! see p107's "v0.2" module doc). Even after that fix, `pad_inset_m` alone
//! still stood every pad apart from its neighbors by default -- an
//! isolated pavilion, always, regardless of whether anything actually
//! called for a gap there. That's the opposite of ordinary urban infill: a
//! block face of buildings sharing party walls, running to the lot line
//! with no gap at all except where a real street, square, or courtyard
//! belongs. (A real example: a mid-rise brick building running the full
//! length of its block with zero setback on the street-facing sides --
//! nothing "standing back" from anything.)
//!
//! `pad_inset_m` is now a construction-joint-sized 0.1m by default -- not
//! a real setback. This operator is what actually decides which pads
//! should merge into one building and which shouldn't: pads whose nearest
//! edges are within `connect_gap_threshold_m` of each other (i.e.
//! separated by nothing but that construction joint) merge; pads
//! separated by a real reserved gap (a street corridor, a P61 square, P37
//! common land -- all of which are subtracted as holes BEFORE pads are
//! seeded, so the resulting gap is much wider than a construction joint)
//! do not.
//!
//! # Approach
//! Runs once, site-scale (`parcel_id == "*"`), over every parcel tagged
//! `p95_building_pad`. Greedily grows clusters: pick an unclustered pad,
//! repeatedly add the nearest unclustered pad within
//! `connect_gap_threshold_m` of ANY pad already in the cluster, up to
//! `max_cluster_pads`. Each cluster of 2+ pads becomes ONE new pad, whose
//! footprint is the CONVEX HULL of every vertex in the cluster (not a
//! true polygon boolean union -- see caveats). Runs BEFORE P96/P107 (not
//! after P107, where Alexander numbers it) so daylight-depth shaping
//! happens on the real, final connected footprint -- shaping each small
//! pad first and then merging already-shaped buildings would compute wing
//! widths for the wrong (unconnected) mass. Documented honestly, same
//! category of pragmatic adaptation as P29's.
//!
//! # What this deliberately does NOT do
//! - Uses the convex hull of the merged pads, not a true polygon boolean
//!   union. Simpler and more robust than implementing general polygon
//!   union, at the cost of a real (bounded) overclaim: any concave notch
//!   between two pads' original shapes gets absorbed into the hull. For
//!   pads that are already roughly rectangular and aligned (the common
//!   case from P95's Voronoi seeding), this overclaim is small; it is NOT
//!   bounded to be small in general.
//! - Doesn't decide WHERE a party wall actually goes (structure, fire
//!   separation, unit boundaries) -- this is geometry only, same
//!   abstraction level as every other operator in this pipeline.
//! - Greedy nearest-first clustering, not a globally optimal grouping.
//!   Cluster membership can depend on iteration order when several pads
//!   are equidistant.
//! - `density_tier`/`target_stories` on a merged pad are inherited from
//!   whichever source pad the cluster started growing from. Pads only
//!   merge when very close, which in practice means they almost always
//!   came from the same P37 block and already share a tier -- but this
//!   isn't verified or reconciled if they don't.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{area, convex_hull, local_to_ring, polygon_min_distance, ring_to_local, Pt2};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Neighborhood, Parcel};
use street_smarts_core::Scope;
use street_smarts_core::opinion::SourceCitation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P108Params {
    /// Max gap between two pads' nearest edges for them to be treated as
    /// "should connect." Sized to catch pads separated only by P95's
    /// pad_inset_m construction joint (2x0.1m = 0.2m combined by default),
    /// not pads separated by a real street/square/common-land gap (several
    /// meters at minimum).
    pub connect_gap_threshold_m: f64,
    /// Cap on how many pads can merge into one building. Alexander's own
    /// text doesn't name a number, but an unbounded merge would eventually
    /// fuse an entire block into one mega-structure -- a real block face
    /// is a RUN of connected buildings, not one building. Default raised
    /// to 24 -- real European block perimeters (the source of this
    /// project's own "barrio" reference) commonly run 20+ party-wall units
    /// deep along one face; 6 was tuned for nothing in particular and cut
    /// real runs short.
    pub max_cluster_pads: f64,
}

impl Parameters for P108Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "connect_gap_threshold_m",
                "Max gap between pads to treat them as should-connect (not a real reserved gap).",
                0.2, 5.0, 1.5,
            ).with_unit("m"),
            ParamSpec::integer(
                "max_cluster_pads",
                "Cap on pads merged into one connected building.",
                2.0, 40.0, 24.0,
            ).with_unit("pads"),
        ]
    }
    fn defaults() -> Self {
        Self { connect_gap_threshold_m: 1.5, max_cluster_pads: 24.0 }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.connect_gap_threshold_m, self.max_cluster_pads]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.connect_gap_threshold_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.max_cluster_pads = s.clamp(*x); }
        p
    }
}

pub struct P108ConnectedBuildings;

impl PatternOperator for P108ConnectedBuildings {
    type Params = P108Params;

    fn name(&self) -> &'static str { "p108_connected_buildings" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p108".into(),
            display: "Alexander et al., A Pattern Language, Pattern 108 (Connected Buildings)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl108/apl108.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Merge neighboring building pads separated by nothing but a construction joint into one continuous party-wall building footprint."
    }

    /// `parcel_id == "*"` (the only mode supported): clusters every pad
    /// tagged `p95_building_pad` in one pass.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p108_connected_buildings only supports parcel_id \"*\" -- it clusters every building pad in one pass.".into());
        }

        let pads: Vec<&Parcel> = nbhd.select(&Scope::BUILDING_PAD).collect();
        if pads.is_empty() {
            return Err("p108_connected_buildings: no building pads found -- run P95 Building Complex first.".into());
        }

        // Project every pad into ONE shared local frame (anchored at the
        // overall centroid) so distances and hulls compare correctly
        // across pad/block boundaries.
        let origin = LngLat::new(
            pads.iter().map(|p| p.polygon.outer.iter().map(|q| q.lng).sum::<f64>() / p.polygon.outer.len() as f64).sum::<f64>() / pads.len() as f64,
            pads.iter().map(|p| p.polygon.outer.iter().map(|q| q.lat).sum::<f64>() / p.polygon.outer.len() as f64).sum::<f64>() / pads.len() as f64,
        );
        let local_pads: Vec<Vec<Pt2>> = pads.iter().map(|p| ring_to_local(&p.polygon.outer, &origin)).collect();

        let n = pads.len();
        let max_cluster = params.max_cluster_pads.round().max(2.0) as usize;
        let mut visited = vec![false; n];
        let mut clusters: Vec<Vec<usize>> = Vec::new();

        for start in 0..n {
            if visited[start] { continue; }
            let mut cluster = vec![start];
            visited[start] = true;
            loop {
                if cluster.len() >= max_cluster { break; }
                // Nearest unvisited pad to ANY pad already in the cluster.
                let mut best: Option<(usize, f64)> = None;
                for &ci in &cluster {
                    for j in 0..n {
                        if visited[j] { continue; }
                        let d = polygon_min_distance(&local_pads[ci], &local_pads[j]);
                        if d <= params.connect_gap_threshold_m {
                            if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                                best = Some((j, d));
                            }
                        }
                    }
                }
                match best {
                    Some((j, _)) => {
                        cluster.push(j);
                        visited[j] = true;
                    }
                    None => break,
                }
            }
            clusters.push(cluster);
        }

        let mut new_parcels: Vec<Parcel> = Vec::new();
        let mut replaced: Vec<String> = Vec::new();
        let mut steps: Vec<String> = Vec::new();
        let mut n_merged_clusters = 0;
        let mut n_pads_merged = 0;
        let mut merged_idx = 0;
        // Each `p108_merged_N`'s source pad ids -- the structured record a
        // lineage walk needs, since the merged id itself discards the
        // `{block_id}_P95_...` naming convention entirely. See
        // `Subdivision::entity_provenance`'s own doc comment and
        // `components.rs`'s `BlockMembership` investigation for why this
        // can't be recovered after the fact from ids alone.
        let mut entity_provenance: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();

        for cluster in &clusters {
            if cluster.len() < 2 {
                continue;
            }
            let mut all_pts: Vec<Pt2> = Vec::new();
            for &i in cluster {
                all_pts.extend(local_pads[i].iter().copied());
            }
            let hull = convex_hull(&all_pts);
            if hull.len() < 3 {
                continue;
            }
            let source_ids: Vec<String> = cluster.iter().map(|&i| pads[i].id.clone()).collect();
            let source_area: f64 = cluster.iter().map(|&i| area(&local_pads[i])).sum();
            let hull_area = area(&hull);

            merged_idx += 1;
            let merged_ring = local_to_ring(&hull, &origin);
            let first = pads[cluster[0]];
            new_parcels.push(Parcel {
                id: format!("p108_merged_{merged_idx}"),
                polygon: street_smarts_core::geometry::Polygon::from_ring(merged_ring),
                area_acres: hull_area / 4046.86,
                use_category: Some("p95_building_pad".into()),
                ownership: None,
                is_eda: true,
                spec: Some(format!("P108_MERGED_{merged_idx}")),
                density_tier: first.density_tier.clone(),
                target_stories: first.target_stories,
            });
            entity_provenance.insert(format!("p108_merged_{merged_idx}"), source_ids.clone());
            replaced.extend(source_ids.iter().cloned());
            n_merged_clusters += 1;
            n_pads_merged += cluster.len();

            steps.push(format!(
                "merged {} pad(s) ({}) into p108_merged_{merged_idx}: {:.0} m² source area -> {:.0} m² hull ({:+.1}% overclaim from convexifying the gap).",
                cluster.len(), source_ids.join(", "), source_area, hull_area,
                if source_area > 0.0 { (hull_area / source_area - 1.0) * 100.0 } else { 0.0 }
            ));
        }

        if n_merged_clusters == 0 {
            return Err(format!(
                "p108_connected_buildings: none of the {} pad(s) were within connect_gap_threshold_m ({:.1}m) of a neighbor -- nothing to connect.",
                pads.len(), params.connect_gap_threshold_m
            ));
        }

        steps.insert(0, format!(
            "{} of {} pad(s) merged into {} connected building(s); {} pad(s) stayed standalone.",
            n_pads_merged, pads.len(), n_merged_clusters, pads.len() - n_pads_merged
        ));

        let trace = SubdivisionTrace {
            operator_name: "p108_connected_buildings".into(),
            operator_source: self.source(),
            headline: format!(
                "Connected {} pad(s) into {} party-wall building(s), {} pad(s) had no close neighbor to join.",
                n_pads_merged, n_merged_clusters, pads.len() - n_pads_merged
            ),
            steps,
            caveats: vec![
                "Merged footprint is the CONVEX HULL of the clustered pads' combined vertices, \
                 not a true polygon boolean union -- simpler and more robust, at the cost of a \
                 real (unbounded in general, usually small for roughly-rectangular aligned pads) \
                 area overclaim in any concave notch between the original pad shapes. Reported \
                 per-cluster in the trace steps above, not hidden.".into(),
                "Greedy nearest-first clustering, not globally optimal -- which pads end up \
                 together can depend on iteration order when several are equidistant.".into(),
                "A merged pad inherits density_tier/target_stories from whichever source pad the \
                 cluster started growing from, not reconciled against its cluster-mates if they \
                 differ (in practice they almost always match, since pads only merge when very \
                 close, which usually means the same P37 block).".into(),
                "This is geometry only -- doesn't decide where a real party wall, fire \
                 separation, or unit boundary would go within the merged footprint.".into(),
            ],
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels,
            new_open_space: vec![],
            new_buildings: vec![],
            new_streets: vec![],
            new_activity_nodes: vec![],
            new_boundaries: vec![],
            replaced_parcel_ids: replaced,
            replaced_open_space_ids: vec![],
            replaced_building_ids: vec![],
            entity_provenance,
            trace,
        })
    }
}
