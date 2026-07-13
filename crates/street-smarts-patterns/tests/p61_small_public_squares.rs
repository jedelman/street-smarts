use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{Neighborhood, NeighborhoodMeta, OpenSpace, OpenSpaceKind};
use street_smarts_patterns::p61_small_public_squares::{P61Params, P61SmallPublicSquares};
use street_smarts_patterns::{Parameters, PatternOperator};

fn square_plaza_neighborhood(side_m: f64) -> Neighborhood {
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
        parcels: vec![],
        buildings: vec![],
        streets: vec![],
        open_space: vec![OpenSpace {
            id: "PLAZA_1".into(),
            polygon: Polygon::from_ring(ring),
            kind: OpenSpaceKind::Plaza,
        }],
        boundaries: vec![],
        activity_nodes: vec![],
        metadata: NeighborhoodMeta {
            source: "synthetic".into(),
            fetched_at: "test".into(),
            license: "test".into(),
            layer_provenance: Default::default(),
            label: "P61 unit fixture".into(),
        },
    }
}

#[test]
fn oversized_plaza_gets_shrunk_and_replaced() {
    // 40m square is well over the 18.3m default threshold.
    let nbhd = square_plaza_neighborhood(40.0);
    let sub = P61SmallPublicSquares.apply(&nbhd, "*", &P61Params::defaults(), 0).unwrap();

    assert_eq!(sub.replaced_open_space_ids, vec!["PLAZA_1".to_string()], "original oversized plaza should be marked for replacement");
    assert_eq!(sub.new_open_space.len(), 1, "should emit exactly one replacement plaza");

    let shrunk = &sub.new_open_space[0];
    let local_area = shrunk.polygon.area_m2();
    // Shrunk to 18.3m linear -> area should be ~18.3^2 = 335 m², not the
    // original 1600 m².
    assert!(local_area < 500.0, "shrunk plaza should be much smaller, got {} m²", local_area);
    assert!(local_area > 250.0, "shouldn't over-shrink either, got {} m²", local_area);
}

#[test]
fn already_compliant_plaza_is_untouched() {
    // 12m square is well under the 18.3m default -- nothing should change.
    let nbhd = square_plaza_neighborhood(12.0);
    let sub = P61SmallPublicSquares.apply(&nbhd, "*", &P61Params::defaults(), 0).unwrap();

    assert!(sub.replaced_open_space_ids.is_empty(), "compliant plaza should not be replaced");
    assert!(sub.new_open_space.is_empty(), "compliant plaza should not get a replacement emitted");
}

#[test]
fn apply_subdivision_actually_removes_the_old_oversized_plaza() {
    use street_smarts_patterns::apply_subdivision;

    let nbhd = square_plaza_neighborhood(40.0);
    let sub = P61SmallPublicSquares.apply(&nbhd, "*", &P61Params::defaults(), 0).unwrap();
    let result = apply_subdivision(&nbhd, &sub);

    // This is the whole point of replaced_open_space_ids: the OLD oversized
    // plaza should be GONE, not sitting alongside the new smaller one.
    assert_eq!(result.open_space.len(), 1, "old oversized plaza should be removed, not duplicated");
    assert_ne!(result.open_space[0].id, "PLAZA_1", "surviving plaza should be the shrunk replacement, not the original");
}

#[test]
fn params_roundtrip() {
    let p = P61Params { max_dimension_m: 15.0, min_meaningful_area_m2: 10.0 };
    let v = p.as_vector();
    let back = P61Params::from_vector(&v);
    assert_eq!(back.max_dimension_m, 15.0);
    assert_eq!(back.min_meaningful_area_m2, 10.0);
}

#[test]
fn real_p95_courtyards_get_evaluated() {
    use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};

    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");

    let p95 = P95BuildingComplex;
    let sub95 = p95.apply(&baseline, "00001129", &P95Params::defaults(), 42).unwrap();
    let with_pads = street_smarts_patterns::apply_subdivision(&baseline, &sub95);

    let sub61 = P61SmallPublicSquares
        .apply(&with_pads, "*", &P61Params::defaults(), 0)
        .unwrap();

    eprintln!(
        "P61 on real P95 courtyards: {}",
        sub61.trace.headline
    );
    // Just needs to run without error on real courtyard geometry -- the
    // real mall parcel's P95 courtyards are typically much larger than
    // 18m, so we expect at least one to need shrinking, but don't assert
    // an exact count since union_pieces' HashMap ordering can shift pad
    // count (and therefore courtyard count/size) run to run.
    assert!(!sub61.trace.steps.is_empty());
}
