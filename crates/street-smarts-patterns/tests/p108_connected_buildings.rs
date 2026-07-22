use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{Neighborhood, NeighborhoodMeta, Parcel};
use street_smarts_patterns::p108_connected_buildings::{P108ConnectedBuildings, P108Params};
use street_smarts_patterns::{Parameters, PatternOperator};

const M_PER_DEG_LNG: f64 = 111_320.0;
const M_PER_DEG_LAT: f64 = 110_540.0;

/// Axis-aligned rectangle pad `width_m` x `depth_m`, with its lower-left
/// corner at local-meter offset (`offset_x_m`, `offset_y_m`) from a shared
/// origin near (0,0) -- lets tests place several pads at known gaps apart.
fn rect_pad(id: &str, offset_x_m: f64, offset_y_m: f64, width_m: f64, depth_m: f64) -> Parcel {
    let to_lnglat = |x_m: f64, y_m: f64| LngLat::new(x_m / M_PER_DEG_LNG, y_m / M_PER_DEG_LAT);
    let ring = vec![
        to_lnglat(offset_x_m, offset_y_m),
        to_lnglat(offset_x_m + width_m, offset_y_m),
        to_lnglat(offset_x_m + width_m, offset_y_m + depth_m),
        to_lnglat(offset_x_m, offset_y_m + depth_m),
    ];
    Parcel {
        id: id.into(),
        polygon: Polygon::from_ring(ring),
        area_acres: (width_m * depth_m) / 4046.86,
        use_category: Some("p95_building_pad".into()),
        ownership: None,
        is_eda: false,
        spec: None,
        density_tier: None,
        target_stories: None,
    }
}

fn nbhd(parcels: Vec<Parcel>) -> Neighborhood {
    Neighborhood {
        id: "test".into(),
        bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
        parcels,
        buildings: vec![],
        streets: vec![],
        open_space: vec![],
        boundaries: vec![],
        activity_nodes: vec![],
        metadata: NeighborhoodMeta {
            source: "synthetic".into(),
            fetched_at: "test".into(),
            license: "test".into(),
            layer_provenance: Default::default(),
            label: "P108 unit fixture".into(),
        },
            pattern_fields: vec![],
        }
}

#[test]
fn two_pads_with_a_tiny_gap_merge_into_one() {
    // 0.2m gap -- exactly what two 0.1m pad_inset_m insets leave behind.
    let n = nbhd(vec![
        rect_pad("A", 0.0, 0.0, 10.0, 10.0),
        rect_pad("B", 10.2, 0.0, 10.0, 10.0),
    ]);
    let sub = P108ConnectedBuildings.apply(&n, "*", &P108Params::defaults(), 1).unwrap();
    assert_eq!(sub.new_parcels.len(), 1, "two close pads should merge into one");
    assert_eq!(sub.replaced_parcel_ids.len(), 2);
    assert!(sub.replaced_parcel_ids.contains(&"A".to_string()));
    assert!(sub.replaced_parcel_ids.contains(&"B".to_string()));

    // Two aligned 10x10 rectangles with a thin gap: convex hull area should
    // be very close to the combined footprint (the hull can't see the thin
    // sliver gap between them since it's below both rectangles' full
    // height, so it barely overclaims here).
    let merged_area = sub.new_parcels[0].polygon.area_m2();
    assert!(merged_area >= 200.0, "merged area should be at least the two pads' combined 200 m², got {merged_area}");
    assert!(merged_area < 205.0, "hull overclaim should be small for two aligned rectangles, got {merged_area}");
}

#[test]
fn two_pads_far_apart_stay_separate() {
    // 20m gap -- a real street-scale separation, well over the default 1.5m threshold.
    let n = nbhd(vec![
        rect_pad("A", 0.0, 0.0, 10.0, 10.0),
        rect_pad("B", 30.0, 0.0, 10.0, 10.0),
    ]);
    let result = P108ConnectedBuildings.apply(&n, "*", &P108Params::defaults(), 1);
    assert!(result.is_err(), "pads separated by a real gap shouldn't merge -- expected an error (nothing to connect)");
}

#[test]
fn a_chain_of_three_close_pads_all_merge_together() {
    let n = nbhd(vec![
        rect_pad("A", 0.0, 0.0, 10.0, 10.0),
        rect_pad("B", 10.2, 0.0, 10.0, 10.0),
        rect_pad("C", 20.4, 0.0, 10.0, 10.0),
    ]);
    let sub = P108ConnectedBuildings.apply(&n, "*", &P108Params::defaults(), 1).unwrap();
    assert_eq!(sub.new_parcels.len(), 1, "a chain of close pads should all merge into one building");
    assert_eq!(sub.replaced_parcel_ids.len(), 3);
}

#[test]
fn max_cluster_pads_caps_the_merge_even_when_more_pads_are_close() {
    // 5 pads in a row, all 0.2m apart -- cap at 2 should produce multiple
    // clusters, not one giant merge.
    let params = P108Params { connect_gap_threshold_m: 1.5, max_cluster_pads: 2.0 };
    let n = nbhd(vec![
        rect_pad("A", 0.0, 0.0, 10.0, 10.0),
        rect_pad("B", 10.2, 0.0, 10.0, 10.0),
        rect_pad("C", 20.4, 0.0, 10.0, 10.0),
        rect_pad("D", 30.6, 0.0, 10.0, 10.0),
        rect_pad("E", 40.8, 0.0, 10.0, 10.0),
    ]);
    let sub = P108ConnectedBuildings.apply(&n, "*", &params, 1).unwrap();
    assert!(sub.new_parcels.len() >= 2, "capping cluster size at 2 pads should produce more than one merged building, got {}", sub.new_parcels.len());
    for p in &sub.new_parcels {
        // Each merged building's own area shouldn't exceed roughly 2 pads' worth.
        assert!(p.polygon.area_m2() < 210.0, "no merged building should exceed the 2-pad cap's area, got {}", p.polygon.area_m2());
    }
}

#[test]
fn a_standalone_pad_with_no_close_neighbor_is_left_untouched() {
    let n = nbhd(vec![
        rect_pad("A", 0.0, 0.0, 10.0, 10.0),
        rect_pad("B", 10.2, 0.0, 10.0, 10.0),
        rect_pad("LONER", 100.0, 100.0, 10.0, 10.0),
    ]);
    let sub = P108ConnectedBuildings.apply(&n, "*", &P108Params::defaults(), 1).unwrap();
    assert_eq!(sub.new_parcels.len(), 1, "only the A+B cluster should produce a merged pad");
    assert!(!sub.replaced_parcel_ids.contains(&"LONER".to_string()), "LONER should not be touched");
}

#[test]
fn merged_pad_inherits_density_tier_and_target_stories_from_the_cluster() {
    let mut a = rect_pad("A", 0.0, 0.0, 10.0, 10.0);
    a.density_tier = Some("core".into());
    a.target_stories = Some(6.0);
    let b = rect_pad("B", 10.2, 0.0, 10.0, 10.0);
    let n = nbhd(vec![a, b]);
    let sub = P108ConnectedBuildings.apply(&n, "*", &P108Params::defaults(), 1).unwrap();
    assert_eq!(sub.new_parcels[0].density_tier.as_deref(), Some("core"));
    assert_eq!(sub.new_parcels[0].target_stories, Some(6.0));
}

#[test]
fn no_building_pads_errors() {
    let n = nbhd(vec![]);
    let result = P108ConnectedBuildings.apply(&n, "*", &P108Params::defaults(), 1);
    assert!(result.is_err());
}

#[test]
fn wrong_parcel_id_mode_errors() {
    let n = nbhd(vec![rect_pad("A", 0.0, 0.0, 10.0, 10.0)]);
    let result = P108ConnectedBuildings.apply(&n, "A", &P108Params::defaults(), 1);
    assert!(result.is_err());
}

#[test]
fn real_mall_parcel_pipeline_produces_at_least_one_connected_building() {
    use street_smarts_patterns::apply_subdivision;
    use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
    use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};

    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");

    let sub37 = P37HouseCluster.apply(&baseline, "00001129", &P37Params::defaults(), 42).unwrap();
    let mut n = apply_subdivision(&baseline, &sub37);

    let block_ids: Vec<String> = n.parcels.iter()
        .filter(|p| p.spec.as_deref().unwrap_or("").starts_with("BLOCK_"))
        .map(|p| p.id.clone())
        .collect();
    for (i, block_id) in block_ids.iter().enumerate() {
        if let Ok(sub95) = P95BuildingComplex.apply(&n, block_id, &P95Params::defaults(), 100 + i as u64) {
            n = apply_subdivision(&n, &sub95);
        }
    }

    let n_pads_before = n.parcels.iter().filter(|p| p.use_category.as_deref() == Some("p95_building_pad")).count();
    assert!(n_pads_before > 0, "P95 should have produced real pads to test P108 against");

    let sub108 = P108ConnectedBuildings.apply(&n, "*", &P108Params::defaults(), 7)
        .expect("with pad_inset_m at its new 0.1m default, real Voronoi-seeded pads should have close neighbors to connect");
    eprintln!(
        "P108 on real mall parcel: {} pads -> {} connected building(s), {} pad(s) replaced",
        n_pads_before, sub108.new_parcels.len(), sub108.replaced_parcel_ids.len()
    );
    assert!(!sub108.new_parcels.is_empty());
}
