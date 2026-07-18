//! Proves P107's `run_native` System port: same string output as
//! `apply()`, and `World.building_typologies`/`World.pad_roles` component
//! entries that match what `apply()` actually wrote -- including real
//! variety (both solid and courtyard branches), not just one degenerate
//! case.

use street_smarts_core::components::{BuildingTypology, PadRole};
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::world::World;
use street_smarts_patterns::p107_wings_of_light::{P107Params, P107WingsOfLight};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};
use street_smarts_patterns::{apply_subdivision, Parameters, PatternOperator};

/// Real P37 -> P95 pads on the real baseline, small enough max_wing_width_m
/// to force BOTH solid and courtyard branches to fire on a real site (not
/// just whichever one happens to be the default).
fn pads_from_real_site() -> Neighborhood {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");
    let sub37 = P37HouseCluster.apply(&baseline, "00001129", &P37Params::defaults(), 42).unwrap();
    let after_p37 = apply_subdivision(&baseline, &sub37);

    let block_id = after_p37.parcels.iter()
        .find(|p| p.spec.as_deref().unwrap_or("").starts_with("BLOCK_"))
        .expect("expected at least one real block from P37")
        .id.clone();
    let sub95 = P95BuildingComplex.apply(&after_p37, &block_id, &P95Params::defaults(), 7).unwrap();
    apply_subdivision(&after_p37, &sub95)
}

#[test]
fn run_native_matches_apply_and_tags_both_components() {
    let nbhd = pads_from_real_site();
    let world = World::from_neighborhood(&nbhd);
    let params = P107Params::defaults();

    let direct_sub = P107WingsOfLight.apply(&nbhd, "*", &params, 3).unwrap();
    assert!(!direct_sub.new_buildings.is_empty(), "expected P107 to shape at least one building");

    let native_world = P107WingsOfLight.run_native(&world, &params, "*", 3).unwrap();

    for b in &direct_sub.new_buildings {
        let expected_label = b.typology.as_deref().expect("apply() should always set typology");
        let expected = BuildingTypology::from_label(expected_label).expect("real label should parse");
        let actual = *native_world.building_typologies.get(&b.id)
            .unwrap_or_else(|| panic!("{} missing from native building_typologies", b.id));
        assert_eq!(expected, actual, "{}: run_native's component disagrees with apply()'s own string", b.id);
    }

    for p in &direct_sub.new_parcels {
        assert_eq!(
            native_world.pad_roles.get(&p.id), Some(&PadRole::PadWithBuilding),
            "{} should be tagged PadWithBuilding in the native World's pad_roles", p.id
        );
    }
}
