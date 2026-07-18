//! Proves P37 and P95's `run_native` System ports: same string output as
//! `apply()`, and a `World.pad_roles` component per new parcel that
//! matches what `apply()` actually wrote.

use street_smarts_core::components::PadRole;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::world::World;
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};
use street_smarts_patterns::{apply_subdivision, Parameters, PatternOperator};

fn eastside_baseline() -> Neighborhood {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    serde_json::from_str(&raw).expect("parseable")
}

#[test]
fn p37_run_native_matches_apply_and_tags_pad_roles() {
    let baseline = eastside_baseline();
    let world = World::from_neighborhood(&baseline);
    let params = P37Params::defaults();

    let direct_sub = P37HouseCluster.apply(&baseline, "00001129", &params, 42).unwrap();
    assert!(!direct_sub.new_parcels.is_empty(), "expected P37 to emit at least one block");

    let native_world = P37HouseCluster.run_native(&world, &params, "00001129", 42).unwrap();
    let native_nbhd = native_world.to_neighborhood();

    let direct_nbhd = apply_subdivision(&baseline, &direct_sub);
    let mut a: Vec<(String, Option<String>)> = direct_nbhd.parcels.iter().map(|p| (p.id.clone(), p.use_category.clone())).collect();
    let mut b: Vec<(String, Option<String>)> = native_nbhd.parcels.iter().map(|p| (p.id.clone(), p.use_category.clone())).collect();
    a.sort();
    b.sort();
    assert_eq!(a, b, "run_native's string output must match apply()'s direct output exactly");

    for p in &direct_sub.new_parcels {
        assert_eq!(
            native_world.pad_roles.get(&p.id), Some(&PadRole::HouseClusterBlock),
            "{} should be tagged HouseClusterBlock in the native World's pad_roles", p.id
        );
    }
}

#[test]
fn p95_run_native_matches_apply_and_tags_pad_roles() {
    let baseline = eastside_baseline();
    let sub37 = P37HouseCluster.apply(&baseline, "00001129", &P37Params::defaults(), 42).unwrap();
    let after_p37 = apply_subdivision(&baseline, &sub37);
    let world = World::from_neighborhood(&after_p37);

    let block_id = after_p37.parcels.iter()
        .find(|p| p.spec.as_deref().unwrap_or("").starts_with("BLOCK_"))
        .expect("expected at least one real block from P37")
        .id.clone();

    let params = P95Params::defaults();
    let direct_sub = P95BuildingComplex.apply(&after_p37, &block_id, &params, 7).unwrap();
    assert!(!direct_sub.new_parcels.is_empty(), "expected P95 to emit at least one pad on a real block");

    let native_world = P95BuildingComplex.run_native(&world, &params, &block_id, 7).unwrap();

    for p in &direct_sub.new_parcels {
        assert_eq!(
            native_world.pad_roles.get(&p.id), Some(&PadRole::BuildingPad),
            "{} should be tagged BuildingPad in the native World's pad_roles", p.id
        );
    }
}
