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
fn oversized_plaza_is_capped_at_max_squares_by_default() {
    // 40m square is well over the 18.3m default threshold. The grid would
    // produce a 3x3 = 9-cell candidate partition, but the default
    // max_squares=4 caps it to "a few," not exhaustive tiling -- the excess
    // candidates are emitted as real Undecided geometry, not more squares.
    let nbhd = square_plaza_neighborhood(40.0);
    let sub = P61SmallPublicSquares.apply(&nbhd, "*", &P61Params::defaults(), 0).unwrap();

    assert_eq!(sub.replaced_open_space_ids, vec!["PLAZA_1".to_string()], "original oversized plaza should be marked for replacement");

    let squares: Vec<_> = sub.new_open_space.iter().filter(|o| o.kind == OpenSpaceKind::Plaza).collect();
    let undecided: Vec<_> = sub.new_open_space.iter().filter(|o| o.kind == OpenSpaceKind::Undecided).collect();
    assert_eq!(sub.new_open_space.len(), squares.len() + undecided.len(), "every emitted open space should be either a kept square or Undecided leftover");
    assert_eq!(squares.len(), 4, "default max_squares=4 should cap the 3x3 candidate grid down to 4 kept squares");
    assert_eq!(undecided.len(), 5, "the other 5 candidates should be emitted as real Undecided geometry, not silently dropped");
    assert_eq!(sub.new_streets.len(), 3, "4 squares should be linked by a 3-edge MST backbone, not a full mesh");

    let mut squares_area = 0.0;
    for sq in &squares {
        let outer = &sq.polygon.outer;
        let min_lng = outer.iter().map(|p| p.lng).fold(f64::INFINITY, f64::min);
        let max_lng = outer.iter().map(|p| p.lng).fold(f64::NEG_INFINITY, f64::max);
        let min_lat = outer.iter().map(|p| p.lat).fold(f64::INFINITY, f64::min);
        let max_lat = outer.iter().map(|p| p.lat).fold(f64::NEG_INFINITY, f64::max);
        let m_per_deg = 111_320.0;
        let width_m = (max_lng - min_lng) * m_per_deg;
        let height_m = (max_lat - min_lat) * m_per_deg;
        assert!(width_m.max(height_m) <= 18.3 + 0.01, "every kept square must comply with the 18.3m cap, got {}", width_m.max(height_m));
        squares_area += sq.polygon.area_m2();
    }
    // 4 squares of ~177.8m² each cover well under the original 1600m².
    assert!(squares_area < 900.0, "capped squares should cover a minority of the original 1600m², got {}", squares_area);

    // Squares + Undecided together should conserve essentially all the
    // original land -- nothing evaporates, it's just tagged by disposition.
    let undecided_area: f64 = undecided.iter().map(|o| o.polygon.area_m2()).sum();
    assert!((squares_area + undecided_area - 1600.0).abs() < 20.0, "squares + Undecided should conserve the original 1600m², got {}", squares_area + undecided_area);

    assert!(sub.trace.steps.iter().any(|s| s.contains("UNCOVERED")), "trace should explicitly report the capped-off land, got: {:?}", sub.trace.steps);

    for street in &sub.new_streets {
        assert_eq!(street.classification.as_deref(), Some("pedestrian"), "connectors between sibling squares should be pedestrian-classified");
    }
}

#[test]
fn raising_max_squares_restores_full_partition_coverage() {
    // Same 40m plaza, but with the cap raised past the 9-cell candidate
    // count -- should behave like the uncapped v0.2 partition and conserve
    // essentially all the original area.
    let nbhd = square_plaza_neighborhood(40.0);
    let params = P61Params { max_squares: 20.0, ..P61Params::defaults() };
    let sub = P61SmallPublicSquares.apply(&nbhd, "*", &params, 0).unwrap();

    assert_eq!(sub.new_open_space.len(), 9, "raising max_squares past the candidate count should uncap the full 3x3 partition");
    assert!(sub.new_open_space.iter().all(|o| o.kind == OpenSpaceKind::Plaza), "with no capping, every candidate should survive as a kept square, not Undecided");
    assert_eq!(sub.new_streets.len(), 8, "9 squares should be linked by an 8-edge MST backbone");

    let total_area: f64 = sub.new_open_space.iter().map(|sq| sq.polygon.area_m2()).sum();
    // Tolerance is loose (~1%) because each sub-square's area_m2() reprojects
    // around its OWN centroid (a slightly different cos(lat) factor per
    // square than the shared local frame used during partitioning), not
    // because any land is actually dropped.
    assert!((total_area - 1600.0).abs() < 20.0, "uncapped partition should conserve the original 1600m² of land, got {}", total_area);
    assert!(!sub.trace.steps.iter().any(|s| s.contains("UNCOVERED")), "uncapped run should not report any capped-off land");
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
    // plaza should be GONE, replaced by the kept squares AND the Undecided
    // leftover the cap produced -- not sitting alongside either of them.
    assert_eq!(result.open_space.len(), 9, "old oversized plaza should be removed and replaced by 4 kept squares + 5 Undecided pieces");
    assert_eq!(result.open_space.iter().filter(|o| o.kind == OpenSpaceKind::Plaza).count(), 4, "4 of the 9 should be kept squares");
    assert_eq!(result.open_space.iter().filter(|o| o.kind == OpenSpaceKind::Undecided).count(), 5, "the other 5 should be real Undecided geometry");
    assert!(result.open_space.iter().all(|o| o.id != "PLAZA_1"), "surviving plazas should be the partitioned replacements, not the original");
    assert_eq!(result.streets.len(), 3, "the MST connector streets should also be merged into the neighborhood");
}

#[test]
fn params_roundtrip() {
    let p = P61Params { max_dimension_m: 15.0, min_meaningful_area_m2: 10.0, connector_width_m: 2.5, max_squares: 6.0 };
    let v = p.as_vector();
    let back = P61Params::from_vector(&v);
    assert_eq!(back.max_dimension_m, 15.0);
    assert_eq!(back.min_meaningful_area_m2, 10.0);
    assert_eq!(back.connector_width_m, 2.5);
    assert_eq!(back.max_squares, 6.0);
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
