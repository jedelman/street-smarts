//! Real integration tests for P29 Density Rings: run P37 on the real
//! baseline mall parcel to get real BLOCK_n geometry, then check P29
//! actually tags every block, assigns a real tier gradient (not one flat
//! value), and respects the site's own core/edge target parameters.

use street_smarts_core::nir::Neighborhood;
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
fn every_block_gets_tagged() {
    let nbhd = blocks_from_real_mall_parcel();
    let n_blocks = nbhd.parcels.iter().filter(|p| p.spec.as_deref().unwrap_or("").starts_with("BLOCK_")).count();
    assert!(n_blocks >= 5);

    let sub = P29DensityRings.apply(&nbhd, "*", &P29Params::defaults(), 7).unwrap();
    assert_eq!(sub.new_parcels.len(), n_blocks);
    assert_eq!(sub.replaced_parcel_ids.len(), n_blocks);
    for p in &sub.new_parcels {
        assert!(p.density_tier.is_some(), "{} should have a density_tier", p.id);
        assert!(p.target_stories.is_some(), "{} should have a target_stories", p.id);
    }
}

#[test]
fn geometry_is_untouched_only_metadata_added() {
    let nbhd = blocks_from_real_mall_parcel();
    let before: std::collections::BTreeMap<String, f64> = nbhd.parcels.iter()
        .filter(|p| p.spec.as_deref().unwrap_or("").starts_with("BLOCK_"))
        .map(|p| (p.id.clone(), p.polygon.area_m2()))
        .collect();

    let sub = P29DensityRings.apply(&nbhd, "*", &P29Params::defaults(), 7).unwrap();
    for p in &sub.new_parcels {
        let before_area = before[&p.id];
        let after_area = p.polygon.area_m2();
        assert!((before_area - after_area).abs() < 1.0, "{}: area should be unchanged, {before_area} vs {after_area}", p.id);
    }
}

#[test]
fn produces_a_real_gradient_not_one_flat_tier() {
    let nbhd = blocks_from_real_mall_parcel();
    let sub = P29DensityRings.apply(&nbhd, "*", &P29Params::defaults(), 7).unwrap();

    let distinct_tiers: std::collections::BTreeSet<String> = sub.new_parcels.iter()
        .filter_map(|p| p.density_tier.clone())
        .collect();
    assert!(distinct_tiers.len() >= 2, "a real site with several blocks should span more than one density tier, got {distinct_tiers:?}");

    let core_stories = sub.new_parcels.iter().find(|p| p.density_tier.as_deref() == Some("core")).and_then(|p| p.target_stories);
    assert_eq!(core_stories, Some(6.0), "the block nearest the density center should get the default core target");
}

#[test]
fn respects_custom_core_and_edge_targets() {
    let nbhd = blocks_from_real_mall_parcel();
    let params = P29Params { n_rings: 2.0, core_target_stories: 10.0, edge_target_stories: 1.0 };
    let sub = P29DensityRings.apply(&nbhd, "*", &params, 7).unwrap();

    let stories: Vec<f64> = sub.new_parcels.iter().filter_map(|p| p.target_stories).collect();
    assert!(stories.iter().any(|&s| (s - 10.0).abs() < 1e-6), "at least one block should hit the core target with 2 rings, got {stories:?}");
    assert!(stories.iter().any(|&s| (s - 1.0).abs() < 1e-6), "at least one block should hit the edge target with 2 rings, got {stories:?}");
}

#[test]
fn wrong_parcel_id_mode_errors_instead_of_silently_doing_nothing() {
    let nbhd = blocks_from_real_mall_parcel();
    let result = P29DensityRings.apply(&nbhd, "some_specific_id", &P29Params::defaults(), 1);
    assert!(result.is_err());
}
