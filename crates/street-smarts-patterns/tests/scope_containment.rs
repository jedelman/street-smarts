//! Scope-containment check: does an operator's returned `Subdivision` only
//! touch entities inside the scope it was invoked on?
//!
//! Deliberately the cheap version -- PATTERN_LANGUAGE_SIMULATION.md §4.3.
//! This is a real, per-operator runtime check against the real Eastside
//! Commons fixture, not a generic property test every operator gets for
//! free (that's PRIMITIVES_SPEC.md §4's `ScopedView`, a bigger later
//! upgrade). Covers P29 here; extending to the other site-scale operators
//! is straightforward (same pattern, different `Scope` variant) and left
//! for a follow-up.
//!
//! P29's own scope used to be `Scope::Block` (it tagged already-carved
//! `BLOCK_n` parcels directly) -- see `PATTERN_ORDERING_AUDIT.md` item 1
//! and `p29_density_rings.rs`'s own module doc for why that changed. It
//! now runs on the RAW site parcel and never touches any parcel at all,
//! which is the strongest possible containment property: zero parcel-level
//! effects, not merely effects confined to one `Scope`.

use street_smarts_core::nir::{Neighborhood, PatternField};
use street_smarts_patterns::p29_density_rings::{P29DensityRings, P29Params};
use street_smarts_patterns::{Parameters, PatternOperator};

#[test]
fn p29_touches_no_parcel_at_all_only_attaches_a_field() {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");

    let sub29 = P29DensityRings
        .apply(&baseline, "00001129", &P29Params::defaults(), 42)
        .expect("P29 should succeed on the real fixture's raw parcel");

    assert!(sub29.new_parcels.is_empty(), "P29 should never produce a new parcel -- it only attaches a field");
    assert!(sub29.replaced_parcel_ids.is_empty(), "P29 should never replace a parcel -- it only attaches a field");
    assert!(sub29.new_buildings.is_empty());
    assert!(sub29.new_open_space.is_empty());
    assert!(sub29.new_streets.is_empty());

    assert_eq!(sub29.new_fields.len(), 1, "P29 should attach exactly one real field");
    let PatternField::Density(field) = &sub29.new_fields[0] else { panic!("expected a Density field") };
    assert!(field.radius_m > 0.0, "the attached field should have a real, positive radius");
}
