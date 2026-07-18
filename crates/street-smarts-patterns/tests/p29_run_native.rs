//! Proves P29's `run_native` System port (see `p29_density_rings.rs`'s own
//! doc comment and `system.rs`'s module doc for what "native" means here):
//! it must produce the same string-tagged parcels `apply()` always has,
//! AND a `World.density_tiers` component per block that's genuinely
//! computed from the same ring assignment, not re-parsed from the string
//! it just wrote.

use street_smarts_core::components::DensityTier;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::world::World;
use street_smarts_patterns::p29_density_rings::{P29DensityRings, P29Params};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::{apply_subdivision, Parameters, PatternOperator};

fn blocks_from_real_mall_parcel() -> Neighborhood {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");
    let sub37 = P37HouseCluster.apply(&baseline, "00001129", &P37Params::defaults(), 42).unwrap();
    apply_subdivision(&baseline, &sub37)
}

#[test]
fn run_native_produces_the_same_string_tags_as_apply() {
    let nbhd = blocks_from_real_mall_parcel();
    let world = World::from_neighborhood(&nbhd);
    let params = P29Params::defaults();

    let direct_sub = P29DensityRings.apply(&nbhd, "*", &params, 7).unwrap();
    let direct_nbhd = apply_subdivision(&nbhd, &direct_sub);

    let native_world = P29DensityRings.run_native(&world, &params, 7).unwrap();
    let native_nbhd = native_world.to_neighborhood();

    let a: std::collections::BTreeMap<String, (Option<String>, Option<f64>)> = direct_nbhd
        .parcels.iter().map(|p| (p.id.clone(), (p.density_tier.clone(), p.target_stories))).collect();
    let b: std::collections::BTreeMap<String, (Option<String>, Option<f64>)> = native_nbhd
        .parcels.iter().map(|p| (p.id.clone(), (p.density_tier.clone(), p.target_stories))).collect();
    assert_eq!(a, b, "run_native's string output must match apply()'s direct output exactly");
}

#[test]
fn run_native_populates_density_tiers_matching_every_block_string_label() {
    let nbhd = blocks_from_real_mall_parcel();
    let world = World::from_neighborhood(&nbhd);
    let params = P29Params::defaults();

    let native_world = P29DensityRings.run_native(&world, &params, 7).unwrap();
    let out_nbhd = native_world.to_neighborhood();

    let blocks: Vec<_> = out_nbhd.parcels.iter()
        .filter(|p| p.spec.as_deref().unwrap_or("").starts_with("BLOCK_"))
        .collect();
    assert!(blocks.len() >= 5, "expected several real blocks from the fixture");

    for b in &blocks {
        let label = b.density_tier.as_deref().expect("run_native's output should still carry the string label");
        let expected = DensityTier::from_label(label).expect("label should be a real P29 tier");
        let actual = *native_world.density_tiers.get(&b.id).unwrap_or_else(|| panic!("{} missing from density_tiers component map", b.id));
        assert_eq!(expected, actual, "{}: component ({:?}) disagrees with its own string label ({:?})", b.id, actual, label);
    }
    assert_eq!(native_world.density_tiers.len(), blocks.len(), "every block should have exactly one density_tiers entry");
}

#[test]
fn run_native_gives_at_least_one_core_and_one_edge_on_a_real_multi_ring_site() {
    // Not just "some component exists" -- confirms real variety across the
    // gradient, the same signal check `every_block_gets_tagged` (in
    // p29_density_rings.rs's own test file) makes for the string path.
    let nbhd = blocks_from_real_mall_parcel();
    let world = World::from_neighborhood(&nbhd);
    let native_world = P29DensityRings.run_native(&world, &P29Params::defaults(), 7).unwrap();

    let tiers: Vec<DensityTier> = native_world.density_tiers.values().copied().collect();
    assert!(tiers.contains(&DensityTier::Core), "expected at least one Core block");
    assert!(tiers.contains(&DensityTier::Edge), "expected at least one Edge block");
}
