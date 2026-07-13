use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{Neighborhood, NeighborhoodMeta, Parcel};
use street_smarts_patterns::p107_wings_of_light::{P107Params, P107WingsOfLight};
use street_smarts_patterns::{Parameters, PatternOperator};

/// Build a single-parcel Neighborhood: an axis-aligned rectangle
/// `width_m` x `depth_m`, tagged as a P95 building pad, anchored near
/// (0,0) lng/lat (fine for local-metres math at this scale -- not real
/// Norfolk coordinates, this is a synthetic unit fixture).
fn rect_pad_neighborhood(width_m: f64, depth_m: f64) -> Neighborhood {
    // ~111,320 m per degree longitude at the equator; good enough for a
    // synthetic small-scale fixture, not meant to be geographically real.
    let m_per_deg = 111_320.0;
    let w = width_m / m_per_deg;
    let d = depth_m / m_per_deg;
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
        use_category: Some("p95_building_pad".into()),
        ownership: None,
        is_eda: false,
        spec: None,
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
