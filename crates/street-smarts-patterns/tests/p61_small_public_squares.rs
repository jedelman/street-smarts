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
fn oversized_plaza_gets_partitioned_into_connected_squares() {
    // 40m square is well over the 18.3m default threshold. ceil(40/18.3) = 3
    // grid cells per axis -> 9 squares of ~13.3m each, all compliant, and
    // an 8-edge MST connecting them (9 points -> 9-1 edges).
    let nbhd = square_plaza_neighborhood(40.0);
    let sub = P61SmallPublicSquares.apply(&nbhd, "*", &P61Params::defaults(), 0).unwrap();

    assert_eq!(sub.replaced_open_space_ids, vec!["PLAZA_1".to_string()], "original oversized plaza should be marked for replacement");
    assert_eq!(sub.new_open_space.len(), 9, "40m square should partition into a 3x3 grid of compliant squares");
    assert_eq!(sub.new_streets.len(), 8, "9 squares should be linked by an 8-edge MST backbone, not a full mesh");

    let mut total_area = 0.0;
    for sq in &sub.new_open_space {
        let outer = &sq.polygon.outer;
        let min_lng = outer.iter().map(|p| p.lng).fold(f64::INFINITY, f64::min);
        let max_lng = outer.iter().map(|p| p.lng).fold(f64::NEG_INFINITY, f64::max);
        let min_lat = outer.iter().map(|p| p.lat).fold(f64::INFINITY, f64::min);
        let max_lat = outer.iter().map(|p| p.lat).fold(f64::NEG_INFINITY, f64::max);
        let m_per_deg = 111_320.0;
        let width_m = (max_lng - min_lng) * m_per_deg;
        let height_m = (max_lat - min_lat) * m_per_deg;
        assert!(width_m.max(height_m) <= 18.3 + 0.01, "every sub-square must comply with the 18.3m cap, got {}", width_m.max(height_m));
        total_area += sq.polygon.area_m2();
    }
    // Grid partition of a plain square conserves area -- no clipping loss.
    // Tolerance is loose (~1%) because each sub-square's area_m2() reprojects
    // around its OWN centroid (a slightly different cos(lat) factor per
    // square than the shared local frame used during partitioning), not
    // because any land is actually dropped.
    assert!((total_area - 1600.0).abs() < 20.0, "partitioned squares should conserve the original 1600m² of land, got {}", total_area);

    for street in &sub.new_streets {
        assert_eq!(street.classification.as_deref(), Some("pedestrian"), "connectors between sibling squares should be pedestrian-classified");
    }
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
    // plaza should be GONE, replaced by the full set of partitioned
    // squares, not sitting alongside them.
    assert_eq!(result.open_space.len(), 9, "old oversized plaza should be removed and replaced by all 9 partitioned squares");
    assert!(result.open_space.iter().all(|o| o.id != "PLAZA_1"), "surviving plazas should be the partitioned replacements, not the original");
    assert_eq!(result.streets.len(), 8, "the MST connector streets should also be merged into the neighborhood");
}

#[test]
fn params_roundtrip() {
    let p = P61Params { max_dimension_m: 15.0, min_meaningful_area_m2: 10.0, connector_width_m: 2.5 };
    let v = p.as_vector();
    let back = P61Params::from_vector(&v);
    assert_eq!(back.max_dimension_m, 15.0);
    assert_eq!(back.min_meaningful_area_m2, 10.0);
    assert_eq!(back.connector_width_m, 2.5);
}

#[test]
fn elongated_plaza_partitions_along_the_long_axis_only() {
    // 60m x 15m: only the long axis exceeds 18.3m. ceil(60/18.3) = 4,
    // ceil(15/18.3) = 1 -> a 4x1 strip of 4 squares, 3 connectors.
    let m_per_deg = 111_320.0;
    let w = 60.0 / m_per_deg;
    let h = 15.0 / m_per_deg;
    let ring = vec![
        LngLat::new(0.0, 0.0),
        LngLat::new(w, 0.0),
        LngLat::new(w, h),
        LngLat::new(0.0, h),
    ];
    let nbhd = Neighborhood {
        id: "test".into(),
        bbox_wgs84: [0.0, 0.0, w, h],
        parcels: vec![],
        buildings: vec![],
        streets: vec![],
        open_space: vec![OpenSpace {
            id: "PLAZA_STRIP".into(),
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
            label: "P61 elongated fixture".into(),
        },
    };

    let sub = P61SmallPublicSquares.apply(&nbhd, "*", &P61Params::defaults(), 0).unwrap();
    assert_eq!(sub.new_open_space.len(), 4, "60m x 15m strip should split 4-wide along the long axis only");
    assert_eq!(sub.new_streets.len(), 3, "4 squares in a line should need exactly 3 connectors (a tree, not a mesh)");
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
