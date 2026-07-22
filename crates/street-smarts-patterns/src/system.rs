//! `System`: an operator that runs against `World` instead of `Neighborhood`
//! directly -- PRIMITIVES_SPEC.md §1.2's Phase B interface, and
//! `IMPLEMENTATION_PLAN.md` Phase 5's "at least P29 is fully ported to a
//! System" exit criterion.
//!
//! # Two ways an operator satisfies `System`
//!
//! **Generic (this module, for free, for all 16 operators).** The blanket
//! `impl<T: DynOperator + ?Sized> System for T` below converts `World` to
//! `Neighborhood` at the boundary, calls the operator's existing, UNCHANGED
//! `apply_json`, applies the returned `Subdivision` via the existing
//! `apply_subdivision`, and converts the result back to `World`. Zero
//! behavior change (it's the same `Neighborhood`-in/`Subdivision`-out path
//! every operator already had, and every existing test for every operator
//! keeps passing unchanged), and it covers literally every operator that
//! implements `DynOperator` -- which is all of them, via `registry.rs`'s
//! own blanket bridge. This is what satisfies PRIMITIVES_SPEC.md §1.3
//! Phase B's "New pattern code queries the typed component; nothing that
//! reads the string field breaks" for the whole operator set at once:
//! `World::from_neighborhood` derives every typed component
//! (`density_tiers`, `pad_roles`, `building_typologies`,
//! `street_classifications`) from whatever strings ANY operator's output
//! contains, so running ANY operator through `System::run` leaves `World`
//! with correct, typed, queryable component state -- not because that
//! operator was individually ported, but because the derivation is a pure
//! function of final string state and is therefore correct regardless of
//! which operator produced it.
//!
//! **Native (a handful of specific operators, each its own follow-up
//! commit).** The generic path derives components by re-deriving them from
//! strings after the fact -- correct, but not what PRIMITIVES_SPEC.md
//! §1.3 literally describes ("new concerns get written to World's
//! component maps AND to their existing string-field shadow... inside
//! `to_neighborhood`" -- components as the primary computation, strings as
//! the shadow, not the other way around). That distinction only actually
//! matters for the operators that ORIGINATE a given field -- P29
//! (`density_tier`), P37/P95 (`use_category`), P107 (`typology`),
//! PathNetwork/P61 (`classification`) -- since every other operator either
//! doesn't touch that field or only reads it.
//!
//! Each of those gets a `run_native` INHERENT method (not a second `impl
//! System`, which would conflict with the generic blanket impl above --
//! `T: DynOperator` already covers every one of these types, and stable
//! Rust has no specialization to let a more specific impl override a
//! blanket one). `run_native` isn't part of any trait; it's an additional,
//! separate entry point the same handful of call sites (`pipeline.rs`,
//! eventually) can choose to use instead of `System::run` when they want
//! the stronger guarantee. Internally, each one shares its ring/pad/
//! typology assignment computation with the operator's own `apply()` via a
//! small extracted helper, so the string label and the typed component are
//! two projections of ONE computation, not one parsed from the other --
//! see each operator's own module for its specific native port and the
//! shared-helper refactor that makes this true rather than aspirational.

use crate::subdivision::{apply_subdivision, DynOperator};
use serde_json::Value as JsonValue;
use street_smarts_core::world::World;

/// An operator that runs against `World`. See this module's own doc
/// comment for the generic-vs-native distinction.
pub trait System {
    fn name(&self) -> &'static str;
    fn run(&self, world: &World, target: &str, params_json: &JsonValue, seed: u64) -> Result<World, String>;
}

/// The generic wrapper -- see module doc. `?Sized` so this also covers
/// `dyn DynOperator` itself (and therefore `Box<dyn DynOperator>` via
/// deref), not just concrete, sized operator types.
impl<T: DynOperator + ?Sized> System for T {
    fn name(&self) -> &'static str {
        DynOperator::name(self)
    }

    fn run(&self, world: &World, target: &str, params_json: &JsonValue, seed: u64) -> Result<World, String> {
        let nbhd = world.to_neighborhood();
        let sub = self.apply_json(&nbhd, target, params_json, seed)?;
        let new_nbhd = apply_subdivision(&nbhd, &sub);
        Ok(World::from_neighborhood(&new_nbhd))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p29_density_rings::P29DensityRings;
    use crate::registry::all_operators_v01;
    use street_smarts_core::nir::Neighborhood;

    fn eastside_baseline_fixture() -> Neighborhood {
        let raw = std::fs::read_to_string("../../data/eastside-baseline.json")
            .expect("fixture present -- run from crates/street-smarts-patterns");
        serde_json::from_str(&raw).expect("parseable")
    }

    #[test]
    fn generic_system_wrapper_matches_direct_apply_subdivision() {
        // Prove the generic path is truly zero-behavior-change: running
        // P29 through System::run and reading its resulting World back out
        // as a Neighborhood must equal running the same operator directly
        // through apply_json + apply_subdivision, the pre-System path
        // every existing caller still uses. P29 now runs on the raw site
        // parcel directly -- no P37 pre-step needed.
        let baseline = eastside_baseline_fixture();
        let world = World::from_neighborhood(&baseline);

        let direct_sub = P29DensityRings
            .apply_json(&baseline, "MILITARY_CIRCLE_ASSEMBLED", &JsonValue::Null, 42)
            .expect("P29 should succeed on the real raw site parcel");
        let direct_nbhd = apply_subdivision(&baseline, &direct_sub);

        let system_world = System::run(&P29DensityRings, &world, "MILITARY_CIRCLE_ASSEMBLED", &JsonValue::Null, 42)
            .expect("System::run should succeed identically");
        let system_nbhd = system_world.to_neighborhood();

        let mut a = direct_nbhd;
        let mut b = system_nbhd;
        a.parcels.sort_by(|x, y| x.id.cmp(&y.id));
        b.parcels.sort_by(|x, y| x.id.cmp(&y.id));
        assert_eq!(a.parcels, b.parcels, "System::run's generic wrapper must produce identical parcel state to the direct apply_subdivision path");
        assert_eq!(a.pattern_fields, b.pattern_fields, "System::run's generic wrapper must produce the same attached field too");
    }

    #[test]
    fn every_registered_operator_is_usable_as_a_system() {
        // "Touches every operator" -- literally: every operator
        // `registry.rs` knows about is `System`-callable via the generic
        // blanket impl, with no per-operator code required for this
        // baseline coverage.
        let baseline = eastside_baseline_fixture();
        let world = World::from_neighborhood(&baseline);
        for op in all_operators_v01() {
            // Not asserting success (many operators only make sense on a
            // specific target/scope, e.g. a block id that doesn't exist on
            // the raw baseline) -- asserting the call is well-typed and
            // returns a Result, i.e. this operator really does implement
            // System via the blanket impl, not asserting every operator
            // succeeds against an arbitrary target.
            let _: Result<World, String> = op.run(&world, "*", &JsonValue::Null, 42);
        }
    }
}
