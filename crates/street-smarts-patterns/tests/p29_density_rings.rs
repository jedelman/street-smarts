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

/// P28 Eccentric Nucleus: with eccentricity_frac > 0, the real Core-tier
/// peak should sit measurably farther from the blocks' own bounding-box
/// center than with eccentricity_frac == 0.0 -- the same "how far from
/// bounding-box center" reasoning p28_eccentric_nucleus's own opinion uses.
#[test]
fn eccentricity_frac_measurably_shifts_the_core_tier_away_from_the_bounding_box_center() {
    let nbhd = blocks_from_real_mall_parcel();

    let block_centroid = |p: &street_smarts_core::nir::Parcel| {
        let lng = p.polygon.outer.iter().map(|q| q.lng).sum::<f64>() / p.polygon.outer.len() as f64;
        let lat = p.polygon.outer.iter().map(|q| q.lat).sum::<f64>() / p.polygon.outer.len() as f64;
        (lng, lat)
    };
    let blocks: Vec<_> = nbhd.parcels.iter().filter(|p| p.spec.as_deref().unwrap_or("").starts_with("BLOCK_")).collect();
    let (min_lng, max_lng) = blocks.iter().map(|p| block_centroid(p).0).fold((f64::MAX, f64::MIN), |(mn, mx), x| (mn.min(x), mx.max(x)));
    let (min_lat, max_lat) = blocks.iter().map(|p| block_centroid(p).1).fold((f64::MAX, f64::MIN), |(mn, mx), x| (mn.min(x), mx.max(x)));
    let bbox_center = ((min_lng + max_lng) / 2.0, (min_lat + max_lat) / 2.0);
    let m_per_deg_lat = 110_540.0;
    let m_per_deg_lng = 111_320.0;
    let dist_from_bbox_center = |lng: f64, lat: f64| {
        let dx = (lng - bbox_center.0) * m_per_deg_lng;
        let dy = (lat - bbox_center.1) * m_per_deg_lat;
        (dx * dx + dy * dy).sqrt()
    };
    let mean_core_dist = |sub: &street_smarts_patterns::subdivision::Subdivision| {
        let core: Vec<_> = sub.new_parcels.iter().filter(|p| p.density_tier.as_deref() == Some("core")).collect();
        let sum: f64 = core.iter().map(|p| { let (lng, lat) = block_centroid(p); dist_from_bbox_center(lng, lat) }).sum();
        sum / core.len().max(1) as f64
    };

    let centered_params = P29Params { eccentricity_frac: 0.0, ..P29Params::defaults() };
    let eccentric_params = P29Params { eccentricity_frac: 0.6, ..P29Params::defaults() };
    let sub_centered = P29DensityRings.apply(&nbhd, "*", &centered_params, 7).unwrap();
    let sub_eccentric = P29DensityRings.apply(&nbhd, "*", &eccentric_params, 7).unwrap();

    let d_centered = mean_core_dist(&sub_centered);
    let d_eccentric = mean_core_dist(&sub_eccentric);
    assert!(
        d_eccentric > d_centered,
        "a higher eccentricity_frac should push the Core tier's mean position farther from the bounding-box center: centered={d_centered:.1}m, eccentric={d_eccentric:.1}m"
    );
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
    let params = P29Params { n_rings: 2.0, core_target_stories: 10.0, edge_target_stories: 1.0, ..P29Params::defaults() };
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
