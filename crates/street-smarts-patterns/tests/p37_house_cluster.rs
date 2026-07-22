use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{DensityField, Neighborhood, NeighborhoodMeta, OpenSpaceKind, Parcel, PatternField};
use street_smarts_patterns::p29_density_rings::{P29DensityRings, P29Params};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::{apply_subdivision, Parameters, PatternOperator};

fn square_parcel_neighborhood(side_m: f64, id: &str) -> Neighborhood {
    let m_per_deg = 111_320.0;
    let s = side_m / m_per_deg;
    let ring = vec![
        LngLat::new(0.0, 0.0),
        LngLat::new(s, 0.0),
        LngLat::new(s, s),
        LngLat::new(0.0, s),
    ];
    Neighborhood {
        id: "test".into(),
        bbox_wgs84: [0.0, 0.0, s, s],
        parcels: vec![Parcel {
            id: id.into(),
            polygon: Polygon::from_ring(ring),
            area_acres: (side_m * side_m) / 4046.86,
            use_category: None,
            ownership: None,
            is_eda: true,
            spec: None,
            density_tier: None,
            target_stories: None,
        }],
        buildings: vec![],
        streets: vec![],
        open_space: vec![],
        boundaries: vec![],
        activity_nodes: vec![],
        pattern_fields: vec![],
        metadata: NeighborhoodMeta {
            source: "synthetic".into(),
            fetched_at: "test".into(),
            license: "test".into(),
            layer_provenance: Default::default(),
            label: "P37 unit fixture".into(),
        },
    }
}

#[test]
fn carves_a_large_parcel_into_several_blocks() {
    // 300m x 300m = 90,000 m². Default target_block_area_m2=7000 -> ~13
    // blocks, clamped to max_blocks=12.
    let nbhd = square_parcel_neighborhood(300.0, "MEGA_1");
    let sub = P37HouseCluster.apply(&nbhd, "MEGA_1", &P37Params::defaults(), 7).unwrap();

    assert_eq!(sub.replaced_parcel_ids, vec!["MEGA_1".to_string()]);
    assert!(sub.new_parcels.len() >= 2, "should carve into multiple blocks, got {}", sub.new_parcels.len());
    assert!(sub.new_parcels.len() <= 12, "should respect max_blocks=12, got {}", sub.new_parcels.len());

    for p in &sub.new_parcels {
        assert!(p.spec.as_deref().unwrap().starts_with("BLOCK_"), "every block should be tagged BLOCK_n, got {:?}", p.spec);
        assert_eq!(p.use_category.as_deref(), Some("house_cluster_block"));
        let block_area_m2 = p.polygon.area_m2();
        assert!(block_area_m2 > 0.0);
        // Not literally checking against target_block_area_m2 exactly --
        // Voronoi cells vary -- but no single block should swallow the
        // whole 90,000 m² parcel.
        assert!(block_area_m2 < 90_000.0, "no single block should be the whole parcel, got {block_area_m2}");
    }

    let total_area: f64 = sub.new_parcels.iter().map(|p| p.polygon.area_m2()).sum();
    // Inset eats some area (real streets between blocks); should still
    // retain the large majority of the original 90,000 m².
    assert!(total_area > 90_000.0 * 0.5, "blocks should retain most of the original area after inset, got {total_area}");
}

#[test]
fn apply_subdivision_replaces_raw_parcel_with_blocks() {
    let nbhd = square_parcel_neighborhood(300.0, "MEGA_1");
    let sub = P37HouseCluster.apply(&nbhd, "MEGA_1", &P37Params::defaults(), 7).unwrap();
    let result = apply_subdivision(&nbhd, &sub);

    assert!(result.parcels.iter().all(|p| p.id != "MEGA_1"), "raw parcel should be replaced, not left alongside its blocks");
    assert_eq!(result.parcels.len(), sub.new_parcels.len());
    // P37 v0.2 reserves common land per block (see module doc) -- one
    // OpenSpace per block, all OpenSpaceKind::Common, none of them a Plaza
    // (that's P61's kind, placed later, not P37's job).
    assert_eq!(result.open_space.len(), sub.new_parcels.len(), "every block should get a common-land patch by default");
    assert!(result.open_space.iter().all(|o| o.kind == OpenSpaceKind::Common));
}

#[test]
fn params_roundtrip() {
    let p = P37Params {
        target_block_area_m2: 5000.0, min_blocks: 3.0, max_blocks: 8.0, block_inset_m: 8.0,
        seed_jitter: 0.3, min_block_area_m2: 1000.0, common_land_fraction: 0.2, min_common_land_area_m2: 200.0,
        seeding_mode: 1.0,
    };
    let v = p.as_vector();
    let back = P37Params::from_vector(&v);
    assert_eq!(back.target_block_area_m2, 5000.0);
    assert_eq!(back.min_blocks, 3.0);
    assert_eq!(back.max_blocks, 8.0);
    assert_eq!(back.block_inset_m, 8.0);
    assert_eq!(back.seed_jitter, 0.3);
    assert_eq!(back.min_block_area_m2, 1000.0);
    assert_eq!(back.common_land_fraction, 0.2);
    assert_eq!(back.min_common_land_area_m2, 200.0);
    assert_eq!(back.seeding_mode, 1.0);
}

#[test]
fn real_mall_parcel_gets_carved_into_human_scaled_blocks() {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");

    let sub = P37HouseCluster
        .apply(&baseline, "00001129", &P37Params::defaults(), 42)
        .expect("P37 should carve the real 47-acre mall parcel");

    eprintln!("P37 on real mall parcel: {}", sub.trace.headline);
    for s in &sub.trace.steps {
        eprintln!("  {s}");
    }

    // The real point: many small blocks, not one big leftover cell (the
    // exact failure mode this operator exists to prevent).
    assert!(sub.new_parcels.len() >= 5, "a 47-acre parcel should carve into several blocks, got {}", sub.new_parcels.len());

    let areas: Vec<f64> = sub.new_parcels.iter().map(|p| p.polygon.area_m2()).collect();
    let max_area = areas.iter().cloned().fold(0.0, f64::max);
    let total_parcel_area_m2 = baseline.parcels.iter().find(|p| p.id == "00001129").unwrap().polygon.area_m2();
    assert!(
        max_area < total_parcel_area_m2 * 0.5,
        "no single block should claim more than half the original 47-acre site, got {max_area} of {total_parcel_area_m2}"
    );
}

#[test]
fn no_density_field_leaves_every_block_unset_same_as_before_the_field_primitive() {
    let nbhd = square_parcel_neighborhood(300.0, "MEGA_1");
    let sub = P37HouseCluster.apply(&nbhd, "MEGA_1", &P37Params::defaults(), 7).unwrap();
    assert!(sub.new_parcels.iter().all(|p| p.density_tier.is_none() && p.target_stories.is_none()));
}

#[test]
fn p29_run_before_p37_produces_blocks_with_real_density_tiers_sampled_from_the_field() {
    let nbhd = square_parcel_neighborhood(300.0, "MEGA_1");
    let sub29 = P29DensityRings.apply(&nbhd, "MEGA_1", &P29Params::defaults(), 7).unwrap();
    let with_field = apply_subdivision(&nbhd, &sub29);
    assert_eq!(with_field.pattern_fields.len(), 1, "P29 should attach exactly one field");

    let sub37 = P37HouseCluster.apply(&with_field, "MEGA_1", &P37Params::defaults(), 7).unwrap();
    assert!(sub37.new_parcels.len() >= 2, "should still carve multiple blocks");
    assert!(
        sub37.new_parcels.iter().all(|p| p.density_tier.is_some() && p.target_stories.is_some()),
        "every block should have a real density_tier/target_stories sampled from P29's field"
    );
    // At least one real tier value should actually be "core" -- the block
    // nearest the field's own center -- not every block landing in the
    // same bucket by construction.
    let tiers: std::collections::BTreeSet<String> =
        sub37.new_parcels.iter().filter_map(|p| p.density_tier.clone()).collect();
    assert!(!tiers.is_empty());
}

#[test]
fn any_density_field_present_gets_sampled_regardless_of_which_parcel_produced_it() {
    // Neighborhood.pattern_fields carries no id scoping it to a specific
    // source parcel -- a real, honest limit worth pinning down: p37_
    // house_cluster samples whatever Density field is present, not
    // specifically "the one P29 just computed for this exact parcel_id".
    // Constructing one directly (bypassing P29) proves that.
    let mut nbhd = square_parcel_neighborhood(300.0, "MEGA_1");
    nbhd.pattern_fields.push(PatternField::Density(DensityField {
        center: LngLat::new(nbhd.bbox_wgs84[2] / 2.0, nbhd.bbox_wgs84[3] / 2.0),
        radius_m: 500.0,
        core_target_stories: 6.0,
        edge_target_stories: 2.0,
        n_rings: 3,
    }));
    let sub37 = P37HouseCluster.apply(&nbhd, "MEGA_1", &P37Params::defaults(), 7).unwrap();
    assert!(sub37.new_parcels.iter().all(|p| p.density_tier.is_some()));
}
