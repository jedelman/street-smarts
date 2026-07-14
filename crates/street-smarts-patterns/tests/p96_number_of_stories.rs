//! Real integration tests for P96 Number of Stories, built on the real
//! P37 -> P29 -> [P61+P95] chain against the baseline mall parcel, so pads
//! carry real, varied density tiers (not synthetic ones).

use street_smarts_core::nir::Neighborhood;
use street_smarts_patterns::p29_density_rings::{P29DensityRings, P29Params};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::p61_small_public_squares::{place_new_squares_n, P61Params, P61SmallPublicSquares};
use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};
use street_smarts_patterns::p96_number_of_stories::{P96NumberOfStories, P96Params};
use street_smarts_patterns::pipeline::allocate_squares_by_area;
use street_smarts_patterns::{apply_subdivision, Parameters, PatternOperator};

fn pads_from_real_mall_parcel() -> Neighborhood {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");

    let sub37 = P37HouseCluster.apply(&baseline, "00001129", &P37Params::defaults(), 42).unwrap();
    let mut nbhd = apply_subdivision(&baseline, &sub37);

    let sub29 = P29DensityRings.apply(&nbhd, "*", &P29Params::defaults(), 42).unwrap();
    nbhd = apply_subdivision(&nbhd, &sub29);

    let block_ids: Vec<String> = nbhd.parcels.iter()
        .filter(|p| p.spec.as_deref().unwrap_or("").starts_with("BLOCK_"))
        .map(|p| p.id.clone())
        .collect();
    let block_areas: Vec<f64> = block_ids.iter()
        .map(|id| nbhd.parcels.iter().find(|p| &p.id == id).unwrap().polygon.area_m2())
        .collect();
    let square_counts = allocate_squares_by_area(&block_areas, 4);

    for (i, block_id) in block_ids.iter().enumerate() {
        let seed = 100 + i as u64;
        if square_counts[i] > 0 {
            let block_parcel = nbhd.parcels.iter().find(|p| &p.id == block_id).unwrap().clone();
            if let Ok(sub61) = place_new_squares_n(&nbhd, &block_parcel, square_counts[i], &P61Params::defaults(), seed, P61SmallPublicSquares.source()) {
                nbhd = apply_subdivision(&nbhd, &sub61);
            }
        }
        if let Ok(sub95) = P95BuildingComplex.apply(&nbhd, block_id, &P95Params::defaults(), seed) {
            nbhd = apply_subdivision(&nbhd, &sub95);
        }
    }
    nbhd
}

#[test]
fn every_pad_gets_a_real_target_stories() {
    let nbhd = pads_from_real_mall_parcel();
    let n_pads = nbhd.parcels.iter().filter(|p| p.use_category.as_deref() == Some("p95_building_pad")).count();
    assert!(n_pads > 0);

    let sub = P96NumberOfStories.apply(&nbhd, "*", &P96Params::defaults(), 11).unwrap();
    assert_eq!(sub.new_parcels.len(), n_pads);
    for p in &sub.new_parcels {
        let s = p.target_stories.expect("every pad should get a target_stories assignment");
        assert!(s >= 1.0, "{}: story count should be at least 1, got {s}", p.id);
    }
}

#[test]
fn honors_the_ordinary_cap_with_only_a_few_widely_spaced_exceptions() {
    let nbhd = pads_from_real_mall_parcel();
    let params = P96Params::defaults();
    let sub = P96NumberOfStories.apply(&nbhd, "*", &params, 11).unwrap();

    let n_pads = sub.new_parcels.len();
    let exceptions: Vec<&street_smarts_core::nir::Parcel> = sub.new_parcels.iter()
        .filter(|p| p.target_stories.unwrap_or(0.0) > params.max_ordinary_stories)
        .collect();

    // Real precedent (Alexander's own "very few... exceptions"): should be
    // a small minority of pads, not the majority.
    assert!(
        (exceptions.len() as f64) <= (n_pads as f64 * 0.3),
        "exceptions should be a small minority, got {}/{}", exceptions.len(), n_pads
    );

    // Pairwise spacing check on whatever exceptions exist.
    let m_per_deg_lat = 110_540.0;
    let m_per_deg_lng = 111_320.0;
    let centroids: Vec<(f64, f64)> = exceptions.iter().map(|p| {
        let lng = p.polygon.outer.iter().map(|q| q.lng).sum::<f64>() / p.polygon.outer.len() as f64;
        let lat = p.polygon.outer.iter().map(|q| q.lat).sum::<f64>() / p.polygon.outer.len() as f64;
        (lng * m_per_deg_lng, lat * m_per_deg_lat)
    }).collect();
    for i in 0..centroids.len() {
        for j in (i + 1)..centroids.len() {
            let dx = centroids[i].0 - centroids[j].0;
            let dy = centroids[i].1 - centroids[j].1;
            let d = (dx * dx + dy * dy).sqrt();
            assert!(d >= params.min_tall_spacing_m - 1.0, "two exceptions should be spaced >= {}m apart, got {d:.1}m", params.min_tall_spacing_m);
        }
    }
}

#[test]
fn no_density_tier_falls_back_to_default_target_stories_uniformly() {
    // A neighborhood where P29 never ran -- pads have no density_tier.
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").unwrap();
    let baseline: Neighborhood = serde_json::from_str(&raw).unwrap();
    let sub37 = street_smarts_patterns::p37_house_cluster::P37HouseCluster
        .apply(&baseline, "00001129", &street_smarts_patterns::p37_house_cluster::P37Params::defaults(), 42).unwrap();
    let mut nbhd = apply_subdivision(&baseline, &sub37);
    let block_id = nbhd.parcels.iter().find(|p| p.spec.as_deref().unwrap_or("").starts_with("BLOCK_")).unwrap().id.clone();
    let sub95 = P95BuildingComplex.apply(&nbhd, &block_id, &P95Params::defaults(), 5).unwrap();
    nbhd = apply_subdivision(&nbhd, &sub95);

    assert!(nbhd.parcels.iter().filter(|p| p.use_category.as_deref() == Some("p95_building_pad")).all(|p| p.density_tier.is_none()));

    let params = P96Params::defaults();
    let sub96 = P96NumberOfStories.apply(&nbhd, "*", &params, 1).unwrap();
    for p in &sub96.new_parcels {
        assert_eq!(p.target_stories, Some(params.default_target_stories), "{}: no tier should fall back to the flat default", p.id);
    }
}

#[test]
fn no_building_pads_errors() {
    let nbhd = pads_from_real_mall_parcel();
    // Strip pads so there's nothing for P96 to work with.
    let mut empty = nbhd.clone();
    empty.parcels.retain(|p| p.use_category.as_deref() != Some("p95_building_pad"));
    let result = P96NumberOfStories.apply(&empty, "*", &P96Params::defaults(), 1);
    assert!(result.is_err());
}
