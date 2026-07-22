//! Proves P37's `run_native` System port (see `p37_house_cluster.rs`'s own
//! "v0.4" module doc and `system.rs`'s module doc for what "native" means
//! here): it must produce the same string-tagged blocks `apply()` always
//! has, AND a `World.density_tiers` component per block that's genuinely
//! computed from the same field sample, not re-parsed from the string it
//! just wrote.
//!
//! This used to be `p29_density_rings`'s own responsibility (see git
//! history before `PATTERN_ORDERING_AUDIT.md` item 1 landed) -- P29 only
//! ever computed a field now, no longer touches any parcel, so it has
//! nothing left for its own native port to dual-write. P37 is the
//! operator that actually samples the field as it individuates each
//! block, so the dual-write proof moved here with it. Was
//! `tests/p29_run_native.rs`, renamed for the same reason.

use street_smarts_core::components::DensityTier;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::world::World;
use street_smarts_patterns::p29_density_rings::{P29DensityRings, P29Params};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::{apply_subdivision, Parameters, PatternOperator};

/// The raw mall parcel with a real P29 field already attached -- the same
/// real state the corrected pipeline gives P37 at its own real position
/// (right after P29, before it carves any block).
fn raw_parcel_with_density_field() -> (Neighborhood, &'static str) {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");
    let parcel_id = "00001129";
    let sub29 = P29DensityRings.apply(&baseline, parcel_id, &P29Params::defaults(), 42).unwrap();
    (apply_subdivision(&baseline, &sub29), parcel_id)
}

#[test]
fn run_native_produces_the_same_string_tags_as_apply() {
    let (nbhd, parcel_id) = raw_parcel_with_density_field();
    let world = World::from_neighborhood(&nbhd);
    let params = P37Params::defaults();

    let direct_sub = P37HouseCluster.apply(&nbhd, parcel_id, &params, 7).unwrap();
    let direct_nbhd = apply_subdivision(&nbhd, &direct_sub);

    let native_world = P37HouseCluster.run_native(&world, &params, parcel_id, 7).unwrap();
    let native_nbhd = native_world.to_neighborhood();

    let a: std::collections::BTreeMap<String, (Option<String>, Option<f64>)> = direct_nbhd
        .parcels.iter().filter(|p| p.spec.as_deref().unwrap_or("").starts_with("BLOCK_"))
        .map(|p| (p.id.clone(), (p.density_tier.clone(), p.target_stories))).collect();
    let b: std::collections::BTreeMap<String, (Option<String>, Option<f64>)> = native_nbhd
        .parcels.iter().filter(|p| p.spec.as_deref().unwrap_or("").starts_with("BLOCK_"))
        .map(|p| (p.id.clone(), (p.density_tier.clone(), p.target_stories))).collect();
    assert_eq!(a, b, "run_native's string output must match apply()'s direct output exactly");
    assert!(!a.is_empty(), "expected several real blocks from the fixture");
}

#[test]
fn run_native_populates_density_tiers_matching_every_block_string_label() {
    let (nbhd, parcel_id) = raw_parcel_with_density_field();
    let world = World::from_neighborhood(&nbhd);
    let params = P37Params::defaults();

    let native_world = P37HouseCluster.run_native(&world, &params, parcel_id, 7).unwrap();
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
fn run_native_gives_real_tier_variety_on_a_real_multi_ring_site() {
    // NOT "at least one Core AND one Edge" -- that guarantee only ever held
    // because the pre-field version measured radius_m from the FARTHEST
    // BLOCK, which tautologically put something at frac=1.0 (Edge) by
    // construction. The field is now computed from the raw site polygon's
    // own vertices, before any block exists -- radius_m reflects the real
    // site's true, often irregular extent (confirmed on this exact real
    // fixture: the 25-parcel MILITARY_CIRCLE_ASSEMBLED footprint's farthest
    // vertex sits ~584m from center, but P37's own Voronoi-carved blocks
    // only ever land within ~371m -- max normalized distance 0.635, never
    // reaching the outer third). That's an honest reflection of a real,
    // irregular site shape, not a bug to paper over by shrinking the
    // radius until a block happens to land past 0.667. What's still real
    // and worth checking: real variety across at least two tiers (not
    // every block landing in the exact same bucket).
    let (nbhd, parcel_id) = raw_parcel_with_density_field();
    let world = World::from_neighborhood(&nbhd);
    let native_world = P37HouseCluster.run_native(&world, &P37Params::defaults(), parcel_id, 7).unwrap();

    let mut distinct_tiers: Vec<DensityTier> = Vec::new();
    for t in native_world.density_tiers.values().copied() {
        if !distinct_tiers.contains(&t) {
            distinct_tiers.push(t);
        }
    }
    assert!(distinct_tiers.contains(&DensityTier::Core), "expected at least one Core block");
    assert!(distinct_tiers.len() >= 2, "expected real variety across at least two density tiers, got only {distinct_tiers:?}");
}

#[test]
fn no_density_field_means_no_density_tiers_component_at_all() {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");
    let world = World::from_neighborhood(&baseline);
    let native_world = P37HouseCluster.run_native(&world, &P37Params::defaults(), "00001129", 7).unwrap();
    assert!(native_world.density_tiers.is_empty(), "no P29 field ever ran -- no block should get a DensityTier component");
}
