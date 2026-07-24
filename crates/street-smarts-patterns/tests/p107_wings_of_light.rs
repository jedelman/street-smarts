use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{Neighborhood, NeighborhoodMeta, Parcel};
use street_smarts_patterns::p107_wings_of_light::{P107Params, P107WingsOfLight};
use street_smarts_patterns::{Parameters, PatternOperator};

/// Build a single-parcel Neighborhood: an axis-aligned rectangle
/// `width_m` x `depth_m`, tagged as a P95 building pad, anchored near
/// (0,0) lng/lat (fine for local-metres math at this scale -- not real
/// Norfolk coordinates, this is a synthetic unit fixture).
fn rect_pad_neighborhood(width_m: f64, depth_m: f64) -> Neighborhood {
    rect_neighborhood(width_m, depth_m, Some("p95_building_pad".into()))
}

fn rect_neighborhood(width_m: f64, depth_m: f64, use_category: Option<String>) -> Neighborhood {
    // Longitude and latitude use DIFFERENT meters-per-degree constants,
    // same as planar.rs's real projection -- using one constant for both
    // axes (as this fixture used to) silently shrinks `depth_m` by ~0.7%
    // once the real code projects it back, which is exactly what tripped
    // up the setback-area assertions below at first.
    let m_per_deg_lng = 111_320.0;
    let m_per_deg_lat = 110_540.0;
    let w = width_m / m_per_deg_lng;
    let d = depth_m / m_per_deg_lat;
    let ring = vec![
        LngLat::new(0.0, 0.0),
        LngLat::new(w, 0.0),
        LngLat::new(w, d),
        LngLat::new(0.0, d),
    ];
    let parcel = Parcel {
        id: "TESTPAD".into(),
        polygon: Polygon::from_ring(ring),
        area_acres: (width_m * depth_m) / 4046.86,
        use_category,
        ownership: None,
        is_eda: false,
        spec: None,
        density_tier: None,
        target_stories: None,
    };
    Neighborhood {
        id: "test".into(),
        bbox_wgs84: [0.0, 0.0, w, d],
        parcels: vec![parcel],
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
            label: "P107 unit fixture".into(),
        },
            pattern_fields: vec![],
        }
}

#[test]
fn narrow_pad_stays_solid() {
    // 12m deep is under the 15m default max_wing_width_m -- no courtyard needed.
    let nbhd = rect_pad_neighborhood(40.0, 12.0);
    let op = P107WingsOfLight;
    let sub = op.apply(&nbhd, "TESTPAD", &P107Params::defaults(), 1).unwrap();
    assert_eq!(sub.new_buildings.len(), 1);
    assert_eq!(sub.new_open_space.len(), 0, "narrow pad shouldn't carve a courtyard");
    assert_eq!(sub.new_buildings[0].typology.as_deref(), Some("p107_solid_v01"));
}

#[test]
fn deep_pad_gets_courtyard_ring() {
    // 40m deep is well over the 15m default -- must carve a courtyard.
    let nbhd = rect_pad_neighborhood(60.0, 40.0);
    let op = P107WingsOfLight;
    let sub = op.apply(&nbhd, "TESTPAD", &P107Params::defaults(), 1).unwrap();
    assert_eq!(sub.new_buildings.len(), 1);
    assert_eq!(sub.new_open_space.len(), 1, "deep pad should carve exactly one courtyard");
    assert_eq!(sub.new_buildings[0].typology.as_deref(), Some("p107_courtyard_v01"));

    let building = &sub.new_buildings[0];
    assert_eq!(building.polygon.holes.len(), 1, "courtyard building should have a hole");

    // Every point in the ring should now be within max_wing_width_m of an
    // exterior wall -- checked indirectly: the hole's area should be
    // meaningfully less than the full envelope (i.e. we actually carved
    // something, not a token sliver).
    let courtyard_area = sub.new_open_space[0].polygon.area_m2();
    assert!(courtyard_area > 30.0, "courtyard should clear courtyard_min_area_m2, got {}", courtyard_area);
}

#[test]
fn p95_pad_gets_no_extra_setback_beyond_its_own_pad_inset() {
    // A P95 pad's own pad_inset_m already reserved its gap -- P107
    // shouldn't inset it again for setback_m. Footprint should equal the
    // full pad geometry (minus nothing), narrow enough to stay solid.
    let nbhd = rect_pad_neighborhood(40.0, 12.0);
    let sub = P107WingsOfLight.apply(&nbhd, "TESTPAD", &P107Params::defaults(), 1).unwrap();
    let footprint_area = sub.new_buildings[0].polygon.area_m2();
    let full_pad_area = 40.0 * 12.0;
    assert!(
        (footprint_area - full_pad_area).abs() < 1.0,
        "P95 pad footprint should equal the full pad ({full_pad_area} m²), no extra setback applied, got {footprint_area} m²"
    );
}

#[test]
fn non_p95_parcel_still_gets_a_real_setback() {
    // A parcel P107 is called on directly (not a P95 pad) has no other
    // reserved gap -- setback_m should still apply, same as before this fix.
    let nbhd = rect_neighborhood(40.0, 12.0, None);
    let params = P107Params::defaults();
    let sub = P107WingsOfLight.apply(&nbhd, "TESTPAD", &params, 1).unwrap();
    let footprint_area = sub.new_buildings[0].polygon.area_m2();
    let full_pad_area = 40.0 * 12.0;
    let expected_after_setback = (40.0 - 2.0 * params.setback_m) * (12.0 - 2.0 * params.setback_m);
    assert!(
        footprint_area < full_pad_area - 1.0,
        "a non-P95 parcel should still lose real area to setback_m, got {footprint_area} vs full {full_pad_area}"
    );
    assert!(
        (footprint_area - expected_after_setback).abs() < 1.0,
        "expected ~{expected_after_setback} m² after a real setback, got {footprint_area} m²"
    );
}

#[test]
fn params_roundtrip() {
    let p = P107Params { max_wing_width_m: 10.0, setback_m: 2.0, ..P107Params::defaults() };
    let v = p.as_vector();
    let back = P107Params::from_vector(&v);
    assert_eq!(back.max_wing_width_m, 10.0);
    assert_eq!(back.setback_m, 2.0);
}

#[test]
fn real_p95_pads_from_mall_parcel_shape_without_error() {
    use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};
    use street_smarts_patterns::apply_subdivision;

    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");

    let p95 = P95BuildingComplex;
    let sub = p95.apply(&baseline, "00001129", &P95Params::defaults(), 42).unwrap();
    let with_pads = apply_subdivision(&baseline, &sub);

    let p107 = P107WingsOfLight;
    let result = p107.apply(&with_pads, "*", &P107Params::defaults(), 42);
    match result {
        Ok(sub107) => {
            eprintln!(
                "P107 on real P95 pads: {} buildings, {} courtyards carved",
                sub107.new_buildings.len(),
                sub107.new_open_space.len()
            );
            assert!(!sub107.new_buildings.is_empty());
        }
        Err(e) => panic!("P107 failed on real P95 output: {e}"),
    }
}
