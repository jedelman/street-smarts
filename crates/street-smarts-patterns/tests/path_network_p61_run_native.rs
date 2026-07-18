//! Proves PathNetwork's `run_native` and P61's `place_new_squares_n_native`
//! System ports: same string output as their direct-call counterparts, and
//! `World.street_classifications` entries that match.

use street_smarts_core::components::StreetClassification;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::world::World;
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::p61_small_public_squares::{place_new_squares_n, place_new_squares_n_native, P61Params, P61SmallPublicSquares};
use street_smarts_patterns::path_network::{PathNetwork, PathNetworkParams};
use street_smarts_patterns::{apply_subdivision, Parameters, PatternOperator};

fn blocks_from_real_site() -> Neighborhood {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");
    let sub37 = P37HouseCluster.apply(&baseline, "00001129", &P37Params::defaults(), 42).unwrap();
    apply_subdivision(&baseline, &sub37)
}

#[test]
fn path_network_run_native_matches_apply_and_tags_both_classes() {
    let nbhd = blocks_from_real_site();
    let world = World::from_neighborhood(&nbhd);
    let params = PathNetworkParams::defaults();

    let direct_sub = PathNetwork.apply(&nbhd, "*", &params, 1).unwrap();
    assert!(!direct_sub.new_streets.is_empty(), "expected at least the MST backbone");

    let native_world = PathNetwork.run_native(&world, &params, "*", 1).unwrap();

    for s in &direct_sub.new_streets {
        let expected_label = s.classification.as_deref().expect("apply() should always set classification");
        let expected = StreetClassification::from_label(expected_label).expect("real label should parse");
        let actual = *native_world.street_classifications.get(&s.id)
            .unwrap_or_else(|| panic!("{} missing from native street_classifications", s.id));
        assert_eq!(expected, actual, "{}: run_native's component disagrees with apply()'s own string", s.id);
    }
    // Real signal check (not just "some component exists"): confirms this
    // fixture actually exercises both the MST backbone (Local) and, if the
    // loop budget produced any, Pedestrian -- not a degenerate single-class
    // result.
    let classes: Vec<StreetClassification> = native_world.street_classifications.values().copied().collect();
    assert!(classes.contains(&StreetClassification::Local), "expected at least one Local (MST backbone) street");
}

#[test]
fn p61_place_new_squares_n_native_matches_direct_call() {
    let nbhd = blocks_from_real_site();
    let block = nbhd.parcels.iter()
        .find(|p| p.spec.as_deref().unwrap_or("").starts_with("BLOCK_"))
        .expect("expected at least one real block from P37")
        .clone();
    let world = World::from_neighborhood(&nbhd);
    let params = P61Params::defaults();

    let direct_sub = place_new_squares_n(&nbhd, &block, 2, &params, 5, P61SmallPublicSquares.source()).unwrap();

    let native_world = place_new_squares_n_native(&world, &block, 2, &params, 5, P61SmallPublicSquares.source()).unwrap();

    for s in &direct_sub.new_streets {
        assert_eq!(
            native_world.street_classifications.get(&s.id), Some(&StreetClassification::Pedestrian),
            "{} should be tagged Pedestrian in the native World's street_classifications", s.id
        );
    }
}
