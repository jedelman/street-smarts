//! Typed components — Phase B of PRIMITIVES_SPEC.md §1's ECS migration.
//!
//! `World` (Phase A) is an id-keyed adapter over `Neighborhood`'s existing
//! `Vec`-based shape; nothing about the schema or serialization changed.
//! This module adds the first real typed component sidecar on top of
//! that -- `DensityTier`, derived (read-only, one direction) from
//! `Parcel.density_tier`'s existing free-form string. The string field
//! stays canonical for serialization; `World.density_tiers` is a coarser,
//! ergonomic, type-checked QUERY view layered over it, not a new source
//! of truth `to_neighborhood` reconstructs a string from.
//!
//! # Why coarse (3 variants), not exact
//!
//! `p29_density_rings`'s real tier vocabulary isn't a fixed 3-value set --
//! it's `"core"` (ring index 0), `"edge"` (ring index `n_rings - 1`), or
//! `"ring_{i}"` for every index in between, and `n_rings` is a tunable
//! parameter (default 3, but not fixed). A `DensityTier` that tried to
//! store the exact `(ring_index, n_rings)` pair recoverable from a bare
//! string would need `n_rings` as context the string alone doesn't carry
//! (the label `"edge"` alone can't tell you whether it came from a
//! 3-ring or 6-ring run). Rather than solve that harder, currently-
//! unneeded problem, `DensityTier` collapses every non-core, non-edge
//! ring into `Middle` -- lossy relative to the string's finer `ring_N`
//! granularity, on purpose, and documented as a deliberate simplification
//! for this first component, not a hidden gap. The string field remains
//! the exact record; `Middle` is "somewhere between," which is what every
//! current consumer (including the `p29_density_rings` opinion's own
//! variance check) actually needs to ask.
//!
//! # `BlockMembership`: answered via history lineage, not a NIR sidecar
//!
//! `PRIMITIVES_SPEC.md` §1.2 names `BlockMembership` as an example
//! component alongside `DensityTier`. Investigated as the natural next
//! pilot after `DensityTier` and NOT built as a parse-from-existing-data
//! sidecar the way `DensityTier` was, for a reason worth recording
//! precisely:
//!
//! `p95_building_complex` pad ids DO encode their source block as a
//! prefix (`format!("{block_id}_P95_courtyard_p{label}")` /
//! `format!("{block_id}_P95_cell_{idx}")`), so parsing block membership
//! from a pad id looks viable at first. But `p108_connected_buildings`
//! -- which runs on every building pad, site-wide, immediately after P95
//! in the real pipeline -- replaces merged pads with a BRAND NEW
//! synthetic id (`format!("p108_merged_{merged_idx}")`) that discards
//! that prefix entirely. By the time the full pipeline finishes, most
//! pads have been through this merge step (see `pipeline.rs`'s own
//! ordering), so a membership map built by parsing ids would be reliably
//! WRONG -- not merely incomplete -- for exactly the pads that matter
//! most, with no signal that anything was lost. That's a worse failure
//! mode than `p29_density_rings`'/`p37_house_cluster`'s own detector
//! opinions hitting `NoView` on final pipeline state (see
//! `check_detector_impact.rs`'s findings) -- `NoView` is an honest "I
//! don't know"; a silently-wrong parsed map is not.
//!
//! There's a deeper reason this can't just be fixed by parsing harder:
//! `p108_connected_buildings` clusters pads by GEOMETRIC ADJACENCY
//! (touching footprints), not by block membership -- nothing stops it
//! from merging two pads that originated from different blocks across a
//! boundary. For a merged pad, "which block does this belong to" may not
//! be a single well-defined value at all, not just a hard-to-recover one.
//!
//! Of the two real paths forward this doc comment originally named
//! (propagate membership FORWARD through P95/P108's own output shapes, or
//! answer it from Phase 4's content-addressed history instead), the
//! history path is what got built: `Subdivision::entity_provenance`
//! records, per new entity id, the source entity id(s) it was derived
//! from (P95: a pad's source is the block it was carved from; P108: a
//! merged pad's sources are every pad clustered into it), `Commit` in
//! `street-smarts-ledger` carries that forward, and
//! `street_smarts_ledger::history::block_membership` walks the commit
//! chain resolving an entity id back to its base source(s) recursively.
//! A pad merged across a block boundary correctly resolves to BOTH source
//! blocks -- not a bug, the honest answer for exactly the case this doc
//! comment flagged as not single-valued. See
//! `street-smarts-ledger/src/history.rs`'s own tests
//! (`block_membership_resolves_a_single_hop_p95_pad_to_its_source_block`,
//! `block_membership_resolves_a_p108_merged_pad_back_through_two_hops`)
//! for this proven end to end against the real P95/P108 operators.
//!
//! Not done as part of this: wiring `street-smarts-patterns::pipeline`'s
//! real 14-step `run_corrected_pipeline_with_p37` through `HistoryStore`.
//! `street-smarts-ledger` already depends on `street-smarts-patterns` (for
//! `DynOperator`/`Subdivision`/`apply_subdivision`), so that orchestration
//! can't live in `pipeline.rs` itself without a dependency cycle -- it
//! would need its own home in `street-smarts-ledger`, and P61's per-block
//! call goes through the free function `place_new_squares_n`, not the
//! `PatternOperator`/`DynOperator` trait `get_or_compute` calls through,
//! so it isn't a drop-in swap of `apply_subdivision` for
//! `get_or_compute`. Real follow-up work, not silently dropped -- the
//! `block_membership` mechanism itself works for any `HistoryStore`-backed
//! commit chain today, it just isn't the one the production pipeline
//! currently builds.

//! # `PadRole`, `BuildingTypology`, `StreetClassification`: the rest of
//! `Parcel.use_category`/`Building.typology`/`Street.classification`
//!
//! Same shape as `DensityTier` above (an `Option<Self>`-returning
//! `from_label`, read-derived by `World::from_neighborhood` from whatever
//! string an operator already wrote -- see `world.rs`), covering the
//! other three of PRIMITIVES_SPEC.md §1.1's five named stringly fields
//! (`Parcel.spec` is the fifth; already served by `Scope`, a Phase 1
//! primitive -- see `scope.rs` -- so it doesn't need a component here).
//!
//! Each vocabulary is the REAL, exhaustive set of values any current
//! operator actually writes (confirmed by grep across
//! `street-smarts-patterns/src`, not assumed):
//! - `PadRole`: `p37_house_cluster` ("house_cluster_block"),
//!   `p95_building_complex` ("p95_building_pad"), and the legacy
//!   `building_shape` stub kept for backward compatibility with the OLD,
//!   pre-corrected pipeline order ("p95_pad_with_building" -- see
//!   `registry.rs`'s own note that this stub isn't part of the real
//!   pipeline).
//! - `BuildingTypology`: `p107_wings_of_light`, the real pipeline's
//!   origin site ("p107_solid_v01" / "p107_solid_fallback_v01" /
//!   "p107_courtyard_v01"), and again the legacy `building_shape` stub
//!   ("p95_inscribed_v01").
//! - `StreetClassification`: `path_network` ("local" / "pedestrian") and
//!   `p61_small_public_squares` ("pedestrian") are the only two real
//!   origin sites today. `Arterial`/`Alley` are included because
//!   `nir.rs`'s own field doc comment names them as the intended full
//!   vocabulary ("arterial", "local", "alley", "pedestrian"), even though
//!   no operator produces them yet -- `from_label` already handles them
//!   correctly the day one does, without a second change here.
//!
//! `System`-native dual-write (component computed directly during
//! generation, not re-derived from the string after the fact) is built
//! for each field's real origin operator in the corrected pipeline --
//! P29 (`DensityTier`), P37 and P95 (`PadRole`), P107
//! (`BuildingTypology`), PathNetwork and P61 (`StreetClassification`) --
//! see each operator's own module for its `impl System` block. The legacy
//! `building_shape` stub is covered only by the generic `System` blanket
//! wrapper (it derives its component correctly via
//! `World::from_neighborhood`, same as every other non-native-ported
//! operator), matching its own de-prioritized status everywhere else in
//! this codebase.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DensityTier {
    Core,
    Middle,
    Edge,
}

impl DensityTier {
    /// Parse `p29_density_rings`'s own label convention. `None` for any
    /// value that isn't `"core"`, `"edge"`, or `"ring_N"` (including the
    /// field simply being absent -- P29 hasn't run, or a fixture predates
    /// it).
    pub fn from_label(label: &str) -> Option<Self> {
        if label == "core" {
            Some(Self::Core)
        } else if label == "edge" {
            Some(Self::Edge)
        } else if label.starts_with("ring_") {
            Some(Self::Middle)
        } else {
            None
        }
    }
}

/// The exact string label for ring index `ring_idx` of `n_rings` total --
/// `p29_density_rings`'s own convention (`"core"` / `"ring_{i}"` /
/// `"edge"`), extracted into one shared function so the generator has a
/// single place that builds this string instead of two independent
/// inline copies of the same if/else (see `p29_density_rings.rs`, which
/// had this logic duplicated at its per-parcel tagging site and its
/// trace-summary site before this).
pub fn ring_tier_label(ring_idx: usize, n_rings: usize) -> String {
    if ring_idx == 0 {
        "core".to_string()
    } else if n_rings > 0 && ring_idx == n_rings - 1 {
        "edge".to_string()
    } else {
        format!("ring_{ring_idx}")
    }
}

/// `Parcel.use_category`'s typed vocabulary -- see this module's own top
/// doc comment for the real origin sites and why `PadWithBuilding` is
/// legacy-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PadRole {
    HouseClusterBlock,
    BuildingPad,
    PadWithBuilding,
}

impl PadRole {
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "house_cluster_block" => Some(Self::HouseClusterBlock),
            "p95_building_pad" => Some(Self::BuildingPad),
            "p95_pad_with_building" => Some(Self::PadWithBuilding),
            _ => None,
        }
    }

    /// The exact string label this variant round-trips to -- `p37_house_cluster`,
    /// `p95_building_complex`'s own convention, used by their native
    /// `System` ports (see each operator's own module) to produce the
    /// component and the string from one shared computation instead of
    /// parsing the string back out after writing it.
    pub fn to_label(self) -> &'static str {
        match self {
            Self::HouseClusterBlock => "house_cluster_block",
            Self::BuildingPad => "p95_building_pad",
            Self::PadWithBuilding => "p95_pad_with_building",
        }
    }
}

/// `Building.typology`'s typed vocabulary -- see this module's own top doc
/// comment for the real origin sites and why `InscribedV01` is legacy-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildingTypology {
    InscribedV01,
    SolidV01,
    SolidFallbackV01,
    CourtyardV01,
}

impl BuildingTypology {
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "p95_inscribed_v01" => Some(Self::InscribedV01),
            "p107_solid_v01" => Some(Self::SolidV01),
            "p107_solid_fallback_v01" => Some(Self::SolidFallbackV01),
            "p107_courtyard_v01" => Some(Self::CourtyardV01),
            _ => None,
        }
    }

    /// The exact string label this variant round-trips to --
    /// `p107_wings_of_light`'s own convention, used by its native `System`
    /// port to produce the component and the string from one shared
    /// computation.
    pub fn to_label(self) -> &'static str {
        match self {
            Self::InscribedV01 => "p95_inscribed_v01",
            Self::SolidV01 => "p107_solid_v01",
            Self::SolidFallbackV01 => "p107_solid_fallback_v01",
            Self::CourtyardV01 => "p107_courtyard_v01",
        }
    }

    /// Whether this typology is a courtyard (ring) building -- the one
    /// predicate repeated as a raw string comparison
    /// (`typology.as_deref() == Some("p107_courtyard_v01")`) across eight+
    /// call sites in `street-smarts-patterns`/`street-smarts-opinions`
    /// before this component existed. Those call sites still compare the
    /// raw `Option<String>` field (this component is additive, not a
    /// replacement for existing read-sites -- see this module's own top
    /// doc comment), but new code should prefer this.
    pub fn is_courtyard(self) -> bool {
        matches!(self, Self::CourtyardV01)
    }
}

/// `Street.classification`'s typed vocabulary -- see this module's own top
/// doc comment for why `Arterial`/`Alley` have no real origin site yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreetClassification {
    Arterial,
    Local,
    Alley,
    Pedestrian,
}

impl StreetClassification {
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "arterial" => Some(Self::Arterial),
            "local" => Some(Self::Local),
            "alley" => Some(Self::Alley),
            "pedestrian" => Some(Self::Pedestrian),
            _ => None,
        }
    }

    /// The exact string label this variant round-trips to --
    /// `path_network`/`p61_small_public_squares`'s own convention, used by
    /// their native `System` ports to produce the component and the
    /// string from one shared computation.
    pub fn to_label(self) -> &'static str {
        match self {
            Self::Arterial => "arterial",
            Self::Local => "local",
            Self::Alley => "alley",
            Self::Pedestrian => "pedestrian",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_label_classifies_the_real_generator_vocabulary() {
        assert_eq!(DensityTier::from_label("core"), Some(DensityTier::Core));
        assert_eq!(DensityTier::from_label("edge"), Some(DensityTier::Edge));
        assert_eq!(DensityTier::from_label("ring_1"), Some(DensityTier::Middle));
        assert_eq!(DensityTier::from_label("ring_4"), Some(DensityTier::Middle));
        assert_eq!(DensityTier::from_label("bogus"), None);
        assert_eq!(DensityTier::from_label(""), None);
    }

    #[test]
    fn ring_tier_label_matches_generator_logic_at_default_n_rings() {
        // n_rings = 3 (P29's own default): indices 0, 1, 2.
        assert_eq!(ring_tier_label(0, 3), "core");
        assert_eq!(ring_tier_label(1, 3), "ring_1");
        assert_eq!(ring_tier_label(2, 3), "edge");
    }

    #[test]
    fn ring_tier_label_handles_single_ring() {
        // n_rings = 1: the only index (0) must read "core", matching the
        // generator's own `if ring_idx == 0` check firing before the
        // `edge` check ever gets evaluated.
        assert_eq!(ring_tier_label(0, 1), "core");
    }

    #[test]
    fn ring_tier_label_handles_many_rings() {
        assert_eq!(ring_tier_label(0, 6), "core");
        assert_eq!(ring_tier_label(1, 6), "ring_1");
        assert_eq!(ring_tier_label(4, 6), "ring_4");
        assert_eq!(ring_tier_label(5, 6), "edge");
    }

    #[test]
    fn every_label_this_function_can_produce_round_trips_through_from_label() {
        for n_rings in [1usize, 2, 3, 6] {
            for ring_idx in 0..n_rings {
                let label = ring_tier_label(ring_idx, n_rings);
                let parsed = DensityTier::from_label(&label);
                assert!(parsed.is_some(), "label {label:?} (ring {ring_idx}/{n_rings}) should parse");
            }
        }
    }

    #[test]
    fn pad_role_classifies_the_real_generator_vocabulary() {
        assert_eq!(PadRole::from_label("house_cluster_block"), Some(PadRole::HouseClusterBlock));
        assert_eq!(PadRole::from_label("p95_building_pad"), Some(PadRole::BuildingPad));
        assert_eq!(PadRole::from_label("p95_pad_with_building"), Some(PadRole::PadWithBuilding));
        assert_eq!(PadRole::from_label("bogus"), None);
        assert_eq!(PadRole::from_label(""), None);
    }

    #[test]
    fn pad_role_round_trips_through_to_label() {
        for variant in [PadRole::HouseClusterBlock, PadRole::BuildingPad, PadRole::PadWithBuilding] {
            assert_eq!(PadRole::from_label(variant.to_label()), Some(variant));
        }
    }

    #[test]
    fn building_typology_classifies_the_real_generator_vocabulary() {
        assert_eq!(BuildingTypology::from_label("p95_inscribed_v01"), Some(BuildingTypology::InscribedV01));
        assert_eq!(BuildingTypology::from_label("p107_solid_v01"), Some(BuildingTypology::SolidV01));
        assert_eq!(BuildingTypology::from_label("p107_solid_fallback_v01"), Some(BuildingTypology::SolidFallbackV01));
        assert_eq!(BuildingTypology::from_label("p107_courtyard_v01"), Some(BuildingTypology::CourtyardV01));
        assert_eq!(BuildingTypology::from_label("bogus"), None);
    }

    #[test]
    fn building_typology_round_trips_through_to_label() {
        for variant in [
            BuildingTypology::InscribedV01,
            BuildingTypology::SolidV01,
            BuildingTypology::SolidFallbackV01,
            BuildingTypology::CourtyardV01,
        ] {
            assert_eq!(BuildingTypology::from_label(variant.to_label()), Some(variant));
        }
    }

    #[test]
    fn building_typology_is_courtyard_matches_only_the_courtyard_variant() {
        assert!(BuildingTypology::CourtyardV01.is_courtyard());
        assert!(!BuildingTypology::SolidV01.is_courtyard());
        assert!(!BuildingTypology::SolidFallbackV01.is_courtyard());
        assert!(!BuildingTypology::InscribedV01.is_courtyard());
    }

    #[test]
    fn street_classification_classifies_the_real_generator_vocabulary() {
        assert_eq!(StreetClassification::from_label("arterial"), Some(StreetClassification::Arterial));
        assert_eq!(StreetClassification::from_label("local"), Some(StreetClassification::Local));
        assert_eq!(StreetClassification::from_label("alley"), Some(StreetClassification::Alley));
        assert_eq!(StreetClassification::from_label("pedestrian"), Some(StreetClassification::Pedestrian));
        assert_eq!(StreetClassification::from_label("bogus"), None);
    }

    #[test]
    fn street_classification_round_trips_through_to_label() {
        for variant in [
            StreetClassification::Arterial,
            StreetClassification::Local,
            StreetClassification::Alley,
            StreetClassification::Pedestrian,
        ] {
            assert_eq!(StreetClassification::from_label(variant.to_label()), Some(variant));
        }
    }
}
