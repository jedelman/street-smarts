//! Scope-containment check: does an operator's returned `Subdivision` only
//! touch entities inside the scope it was invoked on?
//!
//! Deliberately the cheap version -- PATTERN_LANGUAGE_SIMULATION.md §4.3.
//! This is a real, per-operator runtime check against the real Eastside
//! Commons fixture, not a generic property test every operator gets for
//! free (that's PRIMITIVES_SPEC.md §4's `ScopedView`, a bigger later
//! upgrade). Covers P29 (site-scale block tagging: `Scope::Block`) here;
//! extending to the other site-scale operators is straightforward
//! (same pattern, different `Scope` variant) and left for a follow-up.

use street_smarts_core::nir::Neighborhood;
use street_smarts_core::Scope;
use street_smarts_patterns::apply_subdivision;
use street_smarts_patterns::p29_density_rings::{P29DensityRings, P29Params};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::{Parameters, PatternOperator};

#[test]
fn p29_only_replaces_parcels_inside_its_declared_block_scope() {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");

    let sub37 = P37HouseCluster.apply(&baseline, "00001129", &P37Params::defaults(), 42).unwrap();
    let n = apply_subdivision(&baseline, &sub37);

    let block_ids: std::collections::HashSet<String> = n.select_ids(&Scope::Block).into_iter().collect();
    assert!(!block_ids.is_empty(), "P37 should have produced at least one BLOCK_n parcel to test against");

    let non_block_ids: std::collections::HashSet<String> =
        n.parcels.iter().map(|p| p.id.clone()).filter(|id| !block_ids.contains(id)).collect();
    assert!(!non_block_ids.is_empty(), "fixture should have at least one non-block parcel for this test to be meaningful");

    let sub29 = P29DensityRings.apply(&n, "*", &P29Params::defaults(), 42).expect("P29 should succeed on a real fixture with blocks");

    for id in &sub29.replaced_parcel_ids {
        assert!(
            block_ids.contains(id),
            "P29 (declared scope: Scope::Block) replaced parcel {id}, which is not a BLOCK_n \
             parcel -- it wrote outside its declared scope"
        );
    }
    for parcel in &sub29.new_parcels {
        assert!(
            Scope::Block.matches_parcel(parcel),
            "P29 produced a replacement parcel {} that doesn't itself match Scope::Block -- \
             its own output falls outside the scope it's supposed to operate within",
            parcel.id
        );
    }
    assert!(!sub29.replaced_parcel_ids.is_empty(), "P29 should have actually tagged at least one block for this test to be meaningful");
}
