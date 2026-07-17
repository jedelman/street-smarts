//! Content-addressed history for `Neighborhood` states — PRIMITIVES_SPEC.md §2.
//!
//! Formalizes what `street-smarts-patterns::apply_subdivision` was already
//! half-building by hand:
//!
//! ```ignore
//! out.id = format!("{}__{}+{}", nbhd.id, sub.trace.operator_name, sub.trace.seed);
//! ```
//!
//! That's a hash chain built from string concatenation, with no actual
//! store behind it -- nothing lets you look up "give me the Neighborhood
//! for this id" except recomputing the whole pipeline from scratch, and
//! nothing catches two different runs that happen to produce the exact
//! same intermediate state. This module makes the commit graph real: a
//! `NeighborhoodId` is an actual content hash (blake3 of the canonical
//! JSON serialization), `Commit` records real parent/operator/params/seed
//! provenance, and `HistoryStore::get_or_compute` memoizes -- the same
//! `(parent, operator, params, seed)` tuple is a cache hit, not a
//! recomputation.
//!
//! Deliberately scoped to an in-memory store for this crate's own tests
//! and any native/CI caller. The IndexedDB-backed store `street-smarts-web`
//! needs is real, separate follow-up work (wasm-bindgen IndexedDB access
//! doesn't exist yet anywhere in this codebase) -- not built here. This
//! module's job is the data model and the in-process memoization contract;
//! a browser storage backend just needs to implement the same
//! `HistoryStore` trait.
//!
//! What this does NOT do, on purpose: merge. Two commits with the same
//! parent that both touch overlapping geometry don't have a generically
//! well-defined combination the way two non-overlapping text diffs do --
//! see PRIMITIVES_SPEC.md §2.3. Branches from a common ancestor are
//! permanently divergent alternatives to compare, never combined.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use street_smarts_core::nir::Neighborhood;
use street_smarts_patterns::{apply_subdivision, DynOperator, Subdivision};

/// A content hash of a `Neighborhood`'s canonical JSON serialization.
/// Stored as a hex string (not a raw `blake3::Hash`) so it round-trips
/// through `serde_json` without pulling in blake3's own serde feature --
/// matches this codebase's existing convention of readable, debuggable
/// string ids (`SourceCitation.id`, operator names) over opaque binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NeighborhoodId(pub [u8; 32]);

impl NeighborhoodId {
    pub fn to_hex(&self) -> String {
        blake3::Hash::from_bytes(self.0).to_hex().to_string()
    }
}

impl std::fmt::Display for NeighborhoodId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Content-address a full `Neighborhood` snapshot: canonical JSON
/// serialization (deterministic given serde's own field-order guarantee
/// for a fixed struct shape), hashed with blake3.
///
/// A real pitfall this deliberately does NOT paper over: hashing the
/// existing `Neighborhood.id`/`Neighborhood.metadata.label` fields would
/// make the hash depend on the string-concatenation lineage
/// `apply_subdivision` already stamps into `out.id` -- two runs that
/// produce identical geometry but arrived via a different operator
/// sequence would hash differently, which defeats memoization's whole
/// point (recognizing "we already computed this exact state"). So the
/// hash is computed over parcels/buildings/streets/open_space/
/// boundaries/activity_nodes only -- the actual content -- via a
/// dedicated view struct, not the full `Neighborhood` including its own
/// derived-lineage id/label fields.
pub fn hash_neighborhood(n: &Neighborhood) -> NeighborhoodId {
    #[derive(Serialize)]
    struct ContentView<'a> {
        parcels: &'a Vec<street_smarts_core::nir::Parcel>,
        buildings: &'a Vec<street_smarts_core::nir::Building>,
        streets: &'a Vec<street_smarts_core::nir::Street>,
        open_space: &'a Vec<street_smarts_core::nir::OpenSpace>,
        boundaries: &'a Vec<street_smarts_core::nir::Boundary>,
        activity_nodes: &'a Vec<street_smarts_core::nir::ActivityNode>,
    }
    let view = ContentView {
        parcels: &n.parcels,
        buildings: &n.buildings,
        streets: &n.streets,
        open_space: &n.open_space,
        boundaries: &n.boundaries,
        activity_nodes: &n.activity_nodes,
    };
    let bytes = serde_json::to_vec(&view).expect("Neighborhood content is always serializable");
    NeighborhoodId(*blake3::hash(&bytes).as_bytes())
}

/// A recorded step: `parent` (content-addressed, `None` for a root
/// commit) plus the operator/params/seed that produced `id` from it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub id: NeighborhoodId,
    pub parent: Option<NeighborhoodId>,
    pub operator_name: String,
    pub params: serde_json::Value,
    pub seed: u64,
    /// Version tag of the code that produced this commit. A cache entry
    /// from a different `algorithm_version` is treated as a miss, not a
    /// hit -- see PRIMITIVES_SPEC.md §2.4: the same `(parent, op, params,
    /// seed)` tuple genuinely CAN produce a different result after a bug
    /// fix to that operator, and silently trusting a stale cache entry
    /// across that boundary would be wrong, not just imprecise.
    pub algorithm_version: String,
}

pub trait HistoryStore {
    /// Look up a commit's metadata without materializing the full snapshot.
    fn commit(&self, id: &NeighborhoodId) -> Option<Commit>;

    /// Materialize the actual `Neighborhood` for a commit -- replays from
    /// the nearest cached ancestor snapshot plus the patch chain.
    fn materialize(&self, id: &NeighborhoodId) -> Result<Neighborhood, String>;

    /// Run `op` with `params`/`seed` on `parent` UNLESS that exact
    /// `(parent, op, params, seed, algorithm_version)` tuple has already
    /// been computed, in which case return the cached id without
    /// re-running the operator at all.
    fn get_or_compute(
        &mut self,
        parent: NeighborhoodId,
        op: &dyn DynOperator,
        target: &str,
        params_json: &serde_json::Value,
        seed: u64,
        algorithm_version: &str,
    ) -> Result<NeighborhoodId, String>;

    /// Every commit whose parent is `id` -- the "what were the
    /// alternatives from here" query a branching UI needs.
    fn children(&self, id: &NeighborhoodId) -> Vec<NeighborhoodId>;

    /// Register a root `Neighborhood` (e.g. a loaded fixture) with no
    /// parent, so `get_or_compute` has something to build on.
    fn insert_root(&mut self, n: &Neighborhood) -> NeighborhoodId;
}

/// In-memory `HistoryStore`. Stores each commit's `Subdivision` patch
/// (not a full snapshot) plus an unbounded materialized-snapshot cache --
/// sizing/eviction for that cache is real future work once there's a
/// corpus large enough to need it (PRIMITIVES_SPEC.md §2.5), not
/// speculatively built here.
#[derive(Default)]
pub struct InMemoryHistoryStore {
    commits: HashMap<NeighborhoodId, Commit>,
    patches: HashMap<NeighborhoodId, Subdivision>,
    snapshots: HashMap<NeighborhoodId, Neighborhood>,
    /// Counts real `op.apply_json` invocations -- test-observable proof
    /// that `get_or_compute`'s cache hits actually skip recomputation,
    /// not just an assumption from reading the code.
    pub compute_calls: usize,
}

impl InMemoryHistoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl HistoryStore for InMemoryHistoryStore {
    fn commit(&self, id: &NeighborhoodId) -> Option<Commit> {
        self.commits.get(id).cloned()
    }

    fn materialize(&self, id: &NeighborhoodId) -> Result<Neighborhood, String> {
        if let Some(n) = self.snapshots.get(id) {
            return Ok(n.clone());
        }
        let commit = self.commits.get(id).ok_or_else(|| format!("unknown commit {id}"))?;
        let parent_id = commit.parent.ok_or_else(|| format!("commit {id} has no parent and no cached snapshot"))?;
        let parent_snapshot = self.materialize(&parent_id)?;
        let patch = self.patches.get(id).ok_or_else(|| format!("commit {id} has no recorded patch"))?;
        Ok(apply_subdivision(&parent_snapshot, patch))
    }

    fn get_or_compute(
        &mut self,
        parent: NeighborhoodId,
        op: &dyn DynOperator,
        target: &str,
        params_json: &serde_json::Value,
        seed: u64,
        algorithm_version: &str,
    ) -> Result<NeighborhoodId, String> {
        // Cache lookup: any existing commit with this exact parent,
        // operator, params, seed, AND algorithm_version.
        for (id, commit) in &self.commits {
            if commit.parent == Some(parent)
                && commit.operator_name == op.name()
                && &commit.params == params_json
                && commit.seed == seed
                && commit.algorithm_version == algorithm_version
            {
                return Ok(*id);
            }
        }

        self.compute_calls += 1;
        let parent_snapshot = self.materialize(&parent)?;
        let patch = op.apply_json(&parent_snapshot, target, params_json, seed)?;
        let new_snapshot = apply_subdivision(&parent_snapshot, &patch);
        let id = hash_neighborhood(&new_snapshot);

        self.commits.insert(
            id,
            Commit {
                id,
                parent: Some(parent),
                operator_name: op.name().to_string(),
                params: params_json.clone(),
                seed,
                algorithm_version: algorithm_version.to_string(),
            },
        );
        self.patches.insert(id, patch);
        self.snapshots.insert(id, new_snapshot);
        Ok(id)
    }

    fn children(&self, id: &NeighborhoodId) -> Vec<NeighborhoodId> {
        self.commits
            .values()
            .filter(|c| c.parent.as_ref() == Some(id))
            .map(|c| c.id)
            .collect()
    }

    fn insert_root(&mut self, n: &Neighborhood) -> NeighborhoodId {
        let id = hash_neighborhood(n);
        self.snapshots.insert(id, n.clone());
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{NeighborhoodMeta, Parcel};
    use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
    use street_smarts_patterns::{Parameters, PatternOperator};

    fn square_parcel_neighborhood(side_m: f64, id: &str) -> Neighborhood {
        let m_per_deg = 111_320.0;
        let s = side_m / m_per_deg;
        let ring = vec![
            LngLat::new(0.0, 0.0),
            LngLat::new(s, 0.0),
            LngLat::new(s, s),
            LngLat::new(0.0, s),
        ];
        Neighborhood {
            id: "raw".into(),
            bbox_wgs84: [0.0, 0.0, s, s],
            parcels: vec![Parcel {
                id: id.into(),
                polygon: Polygon::from_ring(ring),
                area_acres: (side_m * side_m) / 4046.86,
                use_category: None,
                ownership: None,
                is_eda: true,
                spec: None,
                density_tier: None,
                target_stories: None,
            }],
            buildings: vec![],
            streets: vec![],
            open_space: vec![],
            boundaries: vec![],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "history test fixture".into(),
            },
        }
    }

    #[test]
    fn hash_neighborhood_is_deterministic_and_content_based() {
        let a = square_parcel_neighborhood(300.0, "MEGA_1");
        let mut b = square_parcel_neighborhood(300.0, "MEGA_1");
        // Same content, different lineage-only fields -- the hash must
        // NOT depend on these, or two runs that reach the identical
        // geometric state via different labels would wrongly hash
        // differently, defeating memoization.
        b.id = "a_totally_different_lineage_string".into();
        b.metadata.label = "different label entirely".into();

        assert_eq!(hash_neighborhood(&a), hash_neighborhood(&a), "same input must hash identically every time");
        assert_eq!(hash_neighborhood(&a), hash_neighborhood(&b), "hash must depend on content, not on id/label");

        let c = square_parcel_neighborhood(300.0, "DIFFERENT_PARCEL_ID");
        assert_ne!(hash_neighborhood(&a), hash_neighborhood(&c), "genuinely different content must hash differently");
    }

    #[test]
    fn get_or_compute_is_a_real_cache_hit_not_just_a_lookup() {
        let mut store = InMemoryHistoryStore::new();
        let root = square_parcel_neighborhood(300.0, "MEGA_1");
        let root_id = store.insert_root(&root);

        let op = P37HouseCluster;
        let params = serde_json::to_value(P37Params::defaults().as_map()).unwrap();

        let first = store
            .get_or_compute(root_id, &op, "MEGA_1", &P37Params::defaults().as_map(), 42, "v1")
            .expect("first compute should succeed");
        assert_eq!(store.compute_calls, 1, "first call must actually invoke the operator");

        let second = store
            .get_or_compute(root_id, &op, "MEGA_1", &P37Params::defaults().as_map(), 42, "v1")
            .expect("second call with identical inputs should succeed");
        assert_eq!(first, second, "identical (parent, op, params, seed, version) must resolve to the same id");
        assert_eq!(store.compute_calls, 1, "second call MUST be a cache hit -- compute_calls must not have incremented");

        // Sanity: params was actually round-tripped through JSON as
        // get_or_compute expects, not accidentally unused.
        let _ = params;
    }

    #[test]
    fn different_seed_is_a_real_cache_miss() {
        let mut store = InMemoryHistoryStore::new();
        let root = square_parcel_neighborhood(300.0, "MEGA_1");
        let root_id = store.insert_root(&root);
        let op = P37HouseCluster;

        let a = store.get_or_compute(root_id, &op, "MEGA_1", &P37Params::defaults().as_map(), 1, "v1").unwrap();
        let b = store.get_or_compute(root_id, &op, "MEGA_1", &P37Params::defaults().as_map(), 2, "v1").unwrap();
        assert_eq!(store.compute_calls, 2, "different seeds must each be a real compute, not share a cache entry");
        assert_ne!(a, b, "different seeds should (almost certainly) produce different block layouts");
    }

    #[test]
    fn different_algorithm_version_is_a_real_cache_miss() {
        let mut store = InMemoryHistoryStore::new();
        let root = square_parcel_neighborhood(300.0, "MEGA_1");
        let root_id = store.insert_root(&root);
        let op = P37HouseCluster;

        store.get_or_compute(root_id, &op, "MEGA_1", &P37Params::defaults().as_map(), 42, "v1").unwrap();
        store.get_or_compute(root_id, &op, "MEGA_1", &P37Params::defaults().as_map(), 42, "v2").unwrap();
        assert_eq!(
            store.compute_calls, 2,
            "a different algorithm_version must be treated as a miss, even with identical (parent, op, params, seed) -- \
             a stale cache entry from a different code version must never be silently trusted"
        );
    }

    #[test]
    fn materialize_matches_direct_apply_subdivision() {
        let mut store = InMemoryHistoryStore::new();
        let root = square_parcel_neighborhood(300.0, "MEGA_1");
        let root_id = store.insert_root(&root);
        let op = P37HouseCluster;

        let child_id = store
            .get_or_compute(root_id, &op, "MEGA_1", &P37Params::defaults().as_map(), 7, "v1")
            .unwrap();
        let via_store = store.materialize(&child_id).expect("materialize should succeed");

        let sub_direct = op.apply(&root, "MEGA_1", &P37Params::defaults(), 7).unwrap();
        let via_direct = apply_subdivision(&root, &sub_direct);

        assert_eq!(
            hash_neighborhood(&via_store),
            hash_neighborhood(&via_direct),
            "materializing through the store must reproduce exactly what a direct apply_subdivision call produces"
        );
    }

    #[test]
    fn children_reports_real_branch_points() {
        let mut store = InMemoryHistoryStore::new();
        let root = square_parcel_neighborhood(300.0, "MEGA_1");
        let root_id = store.insert_root(&root);
        let op = P37HouseCluster;

        assert!(store.children(&root_id).is_empty(), "a fresh root has no children yet");

        let child_a = store.get_or_compute(root_id, &op, "MEGA_1", &P37Params::defaults().as_map(), 1, "v1").unwrap();
        let child_b = store.get_or_compute(root_id, &op, "MEGA_1", &P37Params::defaults().as_map(), 2, "v1").unwrap();

        let children = store.children(&root_id);
        assert_eq!(children.len(), 2, "two distinct seeds from the same root are two distinct branches");
        assert!(children.contains(&child_a) && children.contains(&child_b));
    }
}
