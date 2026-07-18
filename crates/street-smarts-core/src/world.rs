//! `World`: an entity-keyed view over `Neighborhood` -- Phase A of
//! PRIMITIVES_SPEC.md §1's ECS migration.
//!
//! Deliberately the smallest possible first step: `Neighborhood`'s own
//! `Vec<Parcel>`/`Vec<Building>`/etc. stay canonical and untouched --
//! nothing about the NIR wire format, the existing fixtures, or any
//! existing operator changes in this phase. `World` is purely a read/write
//! ADAPTER: `from_neighborhood`/`to_neighborhood` convert between the
//! existing Vec-based shape and an id-keyed map shape, so future code
//! (Phase B's typed components, the eventual `System`/query machinery)
//! has an entity-addressable substrate to build on without anyone having
//! to touch the schema first.
//!
//! One deliberate, explicit behavior change worth naming rather than
//! hiding: `Vec` order is not semantically meaningful for an id-keyed
//! entity store, and `to_neighborhood` does not attempt to reproduce the
//! exact original Vec order (it emits in sorted-by-id order, since a
//! `BTreeMap` is what backs each collection here). Round-trip fidelity is
//! therefore checked content-wise (same entities, same field values),
//! not via literal Vec-order-sensitive equality -- see this module's own
//! tests, which normalize by sorting before comparing, and say so.
//!
//! Not built here, on purpose (see PRIMITIVES_SPEC.md §1.3's own phased
//! plan): Phase B's typed components (DensityTier, BlockMembership,
//! PadRole, etc.) as dual-written sidecars, and Phase C's optional
//! deprecation of the shadow string fields. This module is Phase A only.
//!
//! # Review notes (findings, no code changes needed for Phase A)
//!
//! - **`PartialEq` on `f64`-bearing structs is IEEE-754 equality, not a
//!   total equivalence relation.** `NaN != NaN` -- if a `Neighborhood`
//!   ever contained a `NaN` coordinate (degenerate polygon math, a
//!   divide-by-zero upstream), it would compare unequal to an identical
//!   copy of itself, not just fail to round-trip. Nothing in this
//!   codebase currently guarantees NaN can't appear, and this PR doesn't
//!   add that guarantee -- known sharp edge for anyone reaching for
//!   `assert_eq!(Neighborhood, ...)` elsewhere, not something Phase A
//!   needed to fix.
//! - **`BTreeMap` was chosen for test determinism (sorted-by-id output,
//!   see `to_neighborhood_emits_sorted_by_id_not_original_order`), not a
//!   proven requirement of future consumers.** A `HashMap` would be
//!   strictly faster for the stated "O(log n) instead of a linear scan"
//!   goal (O(1) instead of O(log n)) if sorted iteration turns out not to
//!   matter to whatever Phase B's `System`/query machinery actually
//!   needs. Revisit once there's a real consumer, don't assume this
//!   choice is settled.
//! - **Round-trip fidelity here proves CONTENT survives `World`, not that
//!   nothing downstream depends on original `Vec` order for
//!   correctness.** No current risk -- `World` isn't wired into any real
//!   pipeline path in Phase A -- but this hasn't been audited across
//!   P37/P61/P95/etc., and becomes a real question the moment Phase B
//!   routes an actual operator through `World` instead of a `Vec`.

use crate::components::DensityTier;
use crate::nir::{ActivityNode, Boundary, Building, Neighborhood, NeighborhoodMeta, OpenSpace, Parcel, Street};
use std::collections::BTreeMap;

/// An entity-keyed view over a `Neighborhood`. Every collection is a
/// `BTreeMap<id, entity>` instead of a `Vec<entity>` -- O(log n) lookup
/// by id instead of the linear `.iter().find(|p| p.id == x)` scans
/// scattered across the existing pattern operators, and deterministic
/// (sorted-by-id) iteration order as a side effect of using a BTreeMap
/// rather than a HashMap.
///
/// `density_tiers` is Phase B's first typed component sidecar -- see
/// `components.rs`'s own doc comment. Populated by parsing each parcel's
/// EXISTING `density_tier` string in `from_neighborhood`; entries are
/// only present for parcels where that string parses (P29 has run and
/// produced a recognized label). Purely additive and read-only: nothing
/// in `to_neighborhood` reads this map back, the string field remains
/// the one source of truth for serialization.
#[derive(Debug, Clone)]
pub struct World {
    pub id: String,
    pub bbox_wgs84: [f64; 4],
    pub parcels: BTreeMap<String, Parcel>,
    pub buildings: BTreeMap<String, Building>,
    pub streets: BTreeMap<String, Street>,
    pub open_space: BTreeMap<String, OpenSpace>,
    pub boundaries: BTreeMap<String, Boundary>,
    pub activity_nodes: BTreeMap<String, ActivityNode>,
    pub metadata: NeighborhoodMeta,
    pub density_tiers: BTreeMap<String, DensityTier>,
}

impl World {
    pub fn from_neighborhood(n: &Neighborhood) -> Self {
        let density_tiers = n
            .parcels
            .iter()
            .filter_map(|p| {
                let label = p.density_tier.as_deref()?;
                let tier = DensityTier::from_label(label)?;
                Some((p.id.clone(), tier))
            })
            .collect();
        Self {
            id: n.id.clone(),
            bbox_wgs84: n.bbox_wgs84,
            parcels: n.parcels.iter().map(|p| (p.id.clone(), p.clone())).collect(),
            buildings: n.buildings.iter().map(|b| (b.id.clone(), b.clone())).collect(),
            streets: n.streets.iter().map(|s| (s.id.clone(), s.clone())).collect(),
            open_space: n.open_space.iter().map(|o| (o.id.clone(), o.clone())).collect(),
            boundaries: n.boundaries.iter().map(|b| (b.id.clone(), b.clone())).collect(),
            activity_nodes: n.activity_nodes.iter().map(|a| (a.id.clone(), a.clone())).collect(),
            metadata: n.metadata.clone(),
            density_tiers,
        }
    }

    pub fn to_neighborhood(&self) -> Neighborhood {
        Neighborhood {
            id: self.id.clone(),
            bbox_wgs84: self.bbox_wgs84,
            parcels: self.parcels.values().cloned().collect(),
            buildings: self.buildings.values().cloned().collect(),
            streets: self.streets.values().cloned().collect(),
            open_space: self.open_space.values().cloned().collect(),
            boundaries: self.boundaries.values().cloned().collect(),
            activity_nodes: self.activity_nodes.values().cloned().collect(),
            metadata: self.metadata.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Polygon;

    /// Sort every Vec-of-entity field by id, so comparing two
    /// `Neighborhood`s checks "same entities, same field values" without
    /// being sensitive to array position -- the property `World` actually
    /// preserves, not literal Vec-order equality (see this module's own
    /// doc comment for why that distinction is deliberate, not a gap).
    fn normalized(mut n: Neighborhood) -> Neighborhood {
        n.parcels.sort_by(|a, b| a.id.cmp(&b.id));
        n.buildings.sort_by(|a, b| a.id.cmp(&b.id));
        n.streets.sort_by(|a, b| a.id.cmp(&b.id));
        n.open_space.sort_by(|a, b| a.id.cmp(&b.id));
        n.boundaries.sort_by(|a, b| a.id.cmp(&b.id));
        n.activity_nodes.sort_by(|a, b| a.id.cmp(&b.id));
        n
    }

    fn sample_neighborhood() -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![
                Parcel {
                    id: "P2".into(),
                    polygon: Polygon::from_ring(vec![]),
                    area_acres: 1.0,
                    use_category: Some("residential".into()),
                    ownership: None,
                    is_eda: false,
                    spec: Some("BLOCK_1".into()),
                    density_tier: Some("core".into()),
                    target_stories: Some(4.0),
                },
                Parcel {
                    id: "P1".into(),
                    polygon: Polygon::from_ring(vec![]),
                    area_acres: 2.0,
                    use_category: None,
                    ownership: None,
                    is_eda: true,
                    spec: None,
                    density_tier: None,
                    target_stories: None,
                },
            ],
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
                label: "world adapter fixture".into(),
            },
        }
    }

    #[test]
    fn round_trip_preserves_every_entity_and_field() {
        let original = sample_neighborhood();
        let world = World::from_neighborhood(&original);
        let round_tripped = world.to_neighborhood();

        assert_eq!(normalized(original), normalized(round_tripped));
    }

    #[test]
    fn to_neighborhood_emits_sorted_by_id_not_original_order() {
        // Sample data was constructed with P2 before P1 (out of sorted
        // order) specifically to prove this, not by accident.
        let world = World::from_neighborhood(&sample_neighborhood());
        let out = world.to_neighborhood();
        let ids: Vec<&str> = out.parcels.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["P1", "P2"], "World-backed output is sorted-by-id, not insertion order");
    }

    #[test]
    fn density_tiers_sidecar_is_populated_from_the_existing_string_field() {
        // sample_neighborhood: P2 has density_tier "core", P1 has None.
        let world = World::from_neighborhood(&sample_neighborhood());
        assert_eq!(world.density_tiers.get("P2"), Some(&DensityTier::Core));
        assert_eq!(
            world.density_tiers.get("P1"), None,
            "a parcel with no density_tier string should have no sidecar entry, not a default"
        );
        assert_eq!(world.density_tiers.len(), 1, "only parcels with a parseable tier get an entry");
    }

    #[test]
    fn entity_lookup_by_id_is_direct_not_a_linear_scan() {
        let world = World::from_neighborhood(&sample_neighborhood());
        let p = world.parcels.get("P1").expect("P1 should be directly addressable");
        assert_eq!(p.area_acres, 2.0);
        assert!(world.parcels.get("NONEXISTENT").is_none());
    }

    #[test]
    fn round_trips_the_real_eastside_baseline_fixture() {
        let raw = std::fs::read_to_string("../../data/eastside-baseline.json")
            .expect("fixture present -- run from crates/street-smarts-core");
        let original: Neighborhood = serde_json::from_str(&raw).expect("parseable");

        let world = World::from_neighborhood(&original);
        let round_tripped = world.to_neighborhood();

        assert_eq!(normalized(original), normalized(round_tripped), "real fixture must round-trip with identical content");
    }
}
