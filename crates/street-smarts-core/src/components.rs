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
}
