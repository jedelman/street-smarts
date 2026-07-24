use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{Neighborhood, NeighborhoodMeta, OpenSpace, OpenSpaceKind, Parcel, Street};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::p61_small_public_squares::{P61Params, P61SmallPublicSquares};
use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};
use street_smarts_patterns::planar::{area, clip_to_polygon, ring_to_local};
use street_smarts_patterns::{apply_subdivision, Parameters, PatternOperator};

fn square_ring(min_lng: f64, min_lat: f64, side_m: f64) -> Vec<LngLat> {
    let m_per_deg = 111_320.0;
    let s = side_m / m_per_deg;
    vec![
        LngLat::new(min_lng, min_lat),
        LngLat::new(min_lng + s, min_lat),
        LngLat::new(min_lng + s, min_lat + s),
        LngLat::new(min_lng, min_lat + s),
    ]
}

fn raw_parcel(id: &str, ring: Vec<LngLat>) -> Parcel {
    Parcel {
        id: id.into(),
        polygon: Polygon::from_ring(ring),
        area_acres: 0.0,
        use_category: None,
        ownership: None,
        is_eda: true,
        spec: None,
        density_tier: None,
        target_stories: None,
    }
}

fn nbhd(parcels: Vec<Parcel>, open_space: Vec<OpenSpace>) -> Neighborhood {
    Neighborhood {
        id: "test".into(),
        bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
        parcels,
        buildings: vec![],
        streets: vec![],
        open_space,
        boundaries: vec![],
        activity_nodes: vec![],
        metadata: NeighborhoodMeta {
            source: "synthetic".into(),
            fetched_at: "test".into(),
            license: "test".into(),
            layer_provenance: Default::default(),
            label: "P61 raw-placement fixture".into(),
        },
            pattern_fields: vec![],
        }
}

fn nbhd_with_streets(parcels: Vec<Parcel>, open_space: Vec<OpenSpace>, streets: Vec<Street>) -> Neighborhood {
    let mut n = nbhd(parcels, open_space);
    n.streets = streets;
    n
}

#[test]
fn raw_parcel_with_no_plaza_gets_new_squares_placed_directly() {
    let n = nbhd(vec![raw_parcel("RAW_1", square_ring(0.0, 0.0, 100.0))], vec![]);
    let sub = P61SmallPublicSquares.apply(&n, "RAW_1", &P61Params::defaults(), 7).unwrap();

    assert!(sub.replaced_parcel_ids.is_empty(), "raw placement should not replace the parcel itself");
    assert!(!sub.new_open_space.is_empty());
    assert!(sub.new_open_space.iter().all(|o| o.kind == OpenSpaceKind::Plaza));
    assert!(sub.new_open_space.iter().all(|o| o.id.contains("_p61_new_sq")));
    assert!(sub.new_open_space.len() <= 4, "default max_squares=4 should cap direct placement too, got {}", sub.new_open_space.len());

    for sq in &sub.new_open_space {
        let outer = &sq.polygon.outer;
        let min_lng = outer.iter().map(|p| p.lng).fold(f64::INFINITY, f64::min);
        let max_lng = outer.iter().map(|p| p.lng).fold(f64::NEG_INFINITY, f64::max);
        let min_lat = outer.iter().map(|p| p.lat).fold(f64::INFINITY, f64::min);
        let max_lat = outer.iter().map(|p| p.lat).fold(f64::NEG_INFINITY, f64::max);
        // Match planar.rs's own lnglat_to_local constants exactly (111_320
        // for longitude, 110_540 for latitude -- NOT the same value) --
        // using one constant for both axes here previously produced a
        // false-positive ~0.7% overestimate on the latitude axis.
        let width_m = (max_lng - min_lng) * 111_320.0;
        let height_m = (max_lat - min_lat) * 110_540.0;
        let dim = width_m.max(height_m);
        assert!(dim <= 18.3 + 0.01, "directly placed square should already comply, got {dim}m");
    }

    if sub.new_open_space.len() > 1 {
        assert_eq!(sub.new_streets.len(), sub.new_open_space.len() - 1, "squares should be linked by an MST, not a mesh");
        assert!(sub.new_streets.iter().all(|s| s.classification.as_deref() == Some("pedestrian")));
    }
}

/// P126 Something Roughly in the Middle: every newly placed square gets a
/// real ActivityNode, jittered off (not exactly at) the square's own
/// centroid, and still real close to it.
#[test]
fn every_new_square_gets_a_real_activity_node_near_but_not_exactly_at_its_centroid() {
    let n = nbhd(vec![raw_parcel("RAW_1", square_ring(0.0, 0.0, 100.0))], vec![]);
    let sub = P61SmallPublicSquares.apply(&n, "RAW_1", &P61Params::defaults(), 7).unwrap();

    assert_eq!(sub.new_activity_nodes.len(), sub.new_open_space.len(), "every square should get exactly one ActivityNode");

    let m_per_deg_lat = 110_540.0;
    let m_per_deg_lng = 111_320.0;
    for sq in &sub.new_open_space {
        let outer = &sq.polygon.outer;
        let centroid_lng = outer.iter().map(|p| p.lng).sum::<f64>() / outer.len() as f64;
        let centroid_lat = outer.iter().map(|p| p.lat).sum::<f64>() / outer.len() as f64;

        let node = sub.new_activity_nodes.iter()
            .find(|a| a.id.starts_with(&sq.id))
            .unwrap_or_else(|| panic!("expected an ActivityNode for square {}", sq.id));

        let dx = (node.location.lng - centroid_lng) * m_per_deg_lng;
        let dy = (node.location.lat - centroid_lat) * m_per_deg_lat;
        let offset_m = (dx * dx + dy * dy).sqrt();
        assert!(offset_m > 0.01, "{}: ActivityNode should be jittered off the exact centroid, got {offset_m:.3}m offset", sq.id);
        assert!(offset_m < 10.0, "{}: ActivityNode should still sit close to its square, got {offset_m:.1}m offset", sq.id);
    }
}

#[test]
fn star_mode_still_requires_an_existing_plaza() {
    let n = nbhd(vec![raw_parcel("RAW_1", square_ring(0.0, 0.0, 100.0))], vec![]);
    let result = P61SmallPublicSquares.apply(&n, "*", &P61Params::defaults(), 7);
    assert!(result.is_err(), "'*' mode should still require an existing plaza -- unchanged backward-compat behavior");
}

#[test]
fn existing_plaza_for_a_different_parcel_does_not_block_raw_placement() {
    // Parcel A has an existing plaza; parcel B (far away) does not.
    let parcel_a = raw_parcel("A", square_ring(0.0, 0.0, 100.0));
    let parcel_b = raw_parcel("B", square_ring(1.0, 1.0, 100.0)); // ~1 degree away, nowhere near A
    let plaza_for_a = OpenSpace {
        id: "PLAZA_A".into(),
        polygon: Polygon::from_ring(square_ring(0.0004, 0.0004, 15.0)), // inside A's bbox
        kind: OpenSpaceKind::Plaza,
    };
    let n = nbhd(vec![parcel_a, parcel_b], vec![plaza_for_a]);

    let sub = P61SmallPublicSquares.apply(&n, "B", &P61Params::defaults(), 7).unwrap();
    assert!(sub.new_open_space.iter().all(|o| o.id.contains("_p61_new_sq")), "targeting B should place new squares, not touch A's plaza");
}

#[test]
fn existing_plaza_within_the_target_parcel_uses_resize_mode_instead() {
    let parcel_a = raw_parcel("A", square_ring(0.0, 0.0, 200.0));
    let oversized_plaza = OpenSpace {
        id: "PLAZA_A".into(),
        polygon: Polygon::from_ring(square_ring(0.0005, 0.0005, 40.0)), // 40m, oversized, inside A
        kind: OpenSpaceKind::Plaza,
    };
    let n = nbhd(vec![parcel_a], vec![oversized_plaza]);

    let sub = P61SmallPublicSquares.apply(&n, "A", &P61Params::defaults(), 7).unwrap();
    assert_eq!(sub.replaced_open_space_ids, vec!["PLAZA_A".to_string()], "should resize the existing plaza, not place new squares");
    assert!(sub.new_open_space.iter().any(|o| o.id.contains("_p61_sq") && !o.id.contains("_new_")), "resize-mode square naming, not raw-placement naming");
}

#[test]
fn real_p37_then_p61_then_p95_chain_has_zero_overlap() {
    // The actual corrected sequence, end to end, on real data: P37 carves
    // blocks, P61 places squares directly on one block, P95 (reworked)
    // builds pads around them. Proves the three new/reworked pieces from
    // this session actually compose, not just pass in isolation.
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");

    let sub37 = P37HouseCluster.apply(&baseline, "00001129", &P37Params::defaults(), 42).unwrap();
    let with_blocks = apply_subdivision(&baseline, &sub37);

    let block = with_blocks.parcels.iter().find(|p| p.spec.as_deref().unwrap_or("").starts_with("BLOCK_")).expect("P37 should have produced at least one block");
    let block_id = block.id.clone();

    let sub61 = P61SmallPublicSquares.apply(&with_blocks, &block_id, &P61Params::defaults(), 7).unwrap();
    assert!(!sub61.new_open_space.is_empty(), "P61 should place squares on this raw block");
    let with_squares = apply_subdivision(&with_blocks, &sub61);

    let sub95 = P95BuildingComplex.apply(&with_squares, &block_id, &P95Params::defaults(), 3).unwrap();
    assert!(sub95.trace.steps.iter().any(|s| s.contains("reserved hole")), "P95 should report subtracting the squares P61 just placed");

    // Real measured overlap, not a proxy.
    let origin = LngLat::new(
        block.polygon.outer.iter().map(|p| p.lng).sum::<f64>() / block.polygon.outer.len() as f64,
        block.polygon.outer.iter().map(|p| p.lat).sum::<f64>() / block.polygon.outer.len() as f64,
    );
    let mut overlap_area = 0.0;
    for feature in sub95.new_parcels.iter().map(|p| &p.polygon).chain(sub95.new_open_space.iter().map(|o| &o.polygon)) {
        let local_feature = ring_to_local(&feature.outer, &origin);
        for sq in &sub61.new_open_space {
            let local_sq = ring_to_local(&sq.polygon.outer, &origin);
            overlap_area += clip_to_polygon(&local_sq, &local_feature).iter().map(|p| area(p)).sum::<f64>();
        }
    }
    eprintln!(
        "P37->P61->P95 chain on block {}: {} squares placed, P95 produced {} pad(s) + {} courtyard(s), real overlap = {:.2} m²",
        block_id, sub61.new_open_space.len(), sub95.new_parcels.len(), sub95.new_open_space.len(), overlap_area
    );
    assert!(overlap_area < 1.0, "P95 should build around P61's squares with zero real overlap, got {overlap_area} m²");
}

/// P30 Activity Nodes: a real street convergence point (two streets
/// sharing an endpoint) inside a raw parcel should anchor a placed square,
/// not just land at an arbitrary stratified-random position.
#[test]
fn a_real_street_convergence_point_anchors_a_square_there() {
    let m = 1.0 / 111_320.0;
    let parcel = raw_parcel("RAW_1", square_ring(0.0, 0.0, 100.0));

    // Two streets sharing one real endpoint at local (50, 50) inside the
    // 100m parcel -- a real intersection node, the same shape PathNetwork
    // output takes (shared/coincident endpoints, not crossing segments).
    let convergence_lng = 50.0 * m;
    let convergence_lat = 50.0 * m;
    let s1 = Street {
        id: "S1".into(),
        centerline: vec![LngLat::new(10.0 * m, 10.0 * m), LngLat::new(convergence_lng, convergence_lat)],
        classification: Some("local".into()),
        row_width_m: Some(6.0),
        surface: None,
    };
    let s2 = Street {
        id: "S2".into(),
        centerline: vec![LngLat::new(90.0 * m, 10.0 * m), LngLat::new(convergence_lng, convergence_lat)],
        classification: Some("local".into()),
        row_width_m: Some(6.0),
        surface: None,
    };
    let n = nbhd_with_streets(vec![parcel], vec![], vec![s1, s2]);

    let mut params = P61Params::defaults();
    params.max_squares = 1.0;
    let sub = P61SmallPublicSquares.apply(&n, "RAW_1", &params, 7).unwrap();

    assert_eq!(sub.new_open_space.len(), 1, "expected exactly one square with max_squares=1");
    let sq = &sub.new_open_space[0];
    let outer = &sq.polygon.outer;
    let centroid_lng = outer.iter().map(|p| p.lng).sum::<f64>() / outer.len() as f64;
    let centroid_lat = outer.iter().map(|p| p.lat).sum::<f64>() / outer.len() as f64;

    let dx = (centroid_lng - convergence_lng) * 111_320.0;
    let dy = (centroid_lat - convergence_lat) * 110_540.0;
    let offset_m = (dx * dx + dy * dy).sqrt();
    // Not near-zero: the square gets clipped off the street corridor
    // itself (real right-of-way reserved around both streets), so its
    // final centroid sits adjacent to the intersection, not on top of it.
    // Still nowhere near the ~35-70m a stratified-random pick could land
    // at within this 100m parcel.
    assert!(
        offset_m < 15.0,
        "the single placed square should be anchored close to the real street-convergence point \
         (50,50), got centroid offset {offset_m:.1}m instead -- looks like it fell back to \
         stratified-random"
    );
}
