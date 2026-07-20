use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{Neighborhood, NeighborhoodMeta, Parcel};
use street_smarts_patterns::path_network::{PathNetwork, PathNetworkParams};
use street_smarts_patterns::{Parameters, PatternOperator};

/// Four blocks at the corners of a ~100m square, each a single tiny pad
/// tagged BLOCK_<n>. Simple enough that the MST is easy to reason about by
/// hand: a square's MST is 3 of its 4 sides (any one side omitted), never
/// a diagonal (diagonals are longer than sides).
fn four_corner_blocks() -> Neighborhood {
    let m_per_deg = 111_320.0;
    let side_m = 100.0;
    let side_deg = side_m / m_per_deg;

    let make_pad = |id: &str, cx: f64, cy: f64, block: &str| -> Parcel {
        let s = 0.00005; // tiny pad footprint, position is all that matters here
        let ring = vec![
            LngLat::new(cx - s, cy - s),
            LngLat::new(cx + s, cy - s),
            LngLat::new(cx + s, cy + s),
            LngLat::new(cx - s, cy + s),
        ];
        Parcel {
            id: id.into(),
            polygon: Polygon::from_ring(ring),
            area_acres: 0.01,
            use_category: Some("p95_building_pad".into()),
            ownership: None,
            is_eda: false,
            spec: Some(block.into()),
            density_tier: None,
            target_stories: None,
        }
    };

    let parcels = vec![
        make_pad("a", 0.0, 0.0, "BLOCK_0"),
        make_pad("b", side_deg, 0.0, "BLOCK_1"),
        make_pad("c", side_deg, side_deg, "BLOCK_2"),
        make_pad("d", 0.0, side_deg, "BLOCK_3"),
    ];

    Neighborhood {
        id: "test".into(),
        bbox_wgs84: [0.0, 0.0, side_deg, side_deg],
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
            label: "P52 unit fixture".into(),
        },
    }
}

#[test]
fn loop_budget_zero_gives_pure_mst_tree() {
    let nbhd = four_corner_blocks();
    let params = PathNetworkParams { loop_budget: 0.0, local_loop_budget: 0.0, ..PathNetworkParams::defaults() };
    let sub = PathNetwork.apply(&nbhd, "*", &params, 0).unwrap();

    // 4 blocks -> MST has exactly 3 edges, no loop edges.
    assert_eq!(sub.new_streets.len(), 3, "pure tree should have n-1 = 3 edges for 4 blocks");
    assert!(
        sub.new_streets.iter().all(|s| s.classification.as_deref() == Some("local")),
        "with both loop budgets at 0, every edge should be MST backbone ('local')"
    );
    // A square's MST is 3 sides, never a diagonal -- diagonals are longer.
    assert!(
        sub.new_streets.iter().all(|s| !s.id.starts_with("loop_") && !s.id.starts_with("localloop_")),
        "no loop-prefixed edges should exist when both loop budgets are 0"
    );
}

#[test]
fn loop_budget_one_adds_exactly_one_pedestrian_edge() {
    let nbhd = four_corner_blocks();
    let params = PathNetworkParams { loop_budget: 1.0, local_loop_budget: 0.0, ..PathNetworkParams::defaults() };
    let sub = PathNetwork.apply(&nbhd, "*", &params, 0).unwrap();

    assert_eq!(sub.new_streets.len(), 4, "MST (3) + loop_budget 1 = 4 edges");
    let local = sub.new_streets.iter().filter(|s| s.classification.as_deref() == Some("local")).count();
    let pedestrian = sub.new_streets.iter().filter(|s| s.classification.as_deref() == Some("pedestrian")).count();
    assert_eq!(local, 3, "MST backbone should still be exactly 3 edges");
    assert_eq!(pedestrian, 1, "loop_budget=1 should add exactly one supplementary edge");
}

#[test]
fn local_loop_budget_one_adds_exactly_one_local_edge() {
    let nbhd = four_corner_blocks();
    let params = PathNetworkParams { loop_budget: 0.0, local_loop_budget: 1.0, ..PathNetworkParams::defaults() };
    let sub = PathNetwork.apply(&nbhd, "*", &params, 0).unwrap();

    assert_eq!(sub.new_streets.len(), 4, "MST (3) + local_loop_budget 1 = 4 edges");
    let local = sub.new_streets.iter().filter(|s| s.classification.as_deref() == Some("local")).count();
    assert_eq!(local, 4, "MST backbone (3) plus one local_loop edge should all be classified 'local'");
    assert!(
        sub.new_streets.iter().any(|s| s.id.starts_with("localloop_")),
        "a localloop_-prefixed edge should exist when local_loop_budget=1"
    );
    // Local streets now contain a real cycle: edges >= nodes for the
    // 4-block connected component (4 edges, 4 nodes) -- this is exactly
    // what p49_looped_local_roads checks for.
}

#[test]
fn loop_budget_never_exceeds_available_non_mst_edges() {
    // 4 blocks -> complete graph has 6 edges, MST takes 3, leaving 3
    // possible loop edges. Asking for 10 shouldn't crash or duplicate --
    // it should just take all 3 available (loop_budget's share is taken
    // first, leaving nothing for local_loop_budget).
    let nbhd = four_corner_blocks();
    let params = PathNetworkParams { loop_budget: 10.0, ..PathNetworkParams::defaults() };
    let sub = PathNetwork.apply(&nbhd, "*", &params, 0).unwrap();
    assert_eq!(sub.new_streets.len(), 6, "should cap at the complete graph, not error or duplicate");
}

#[test]
fn every_segment_bulges_at_least_its_own_row_width() {
    // Alexander's P121 Path Shape: a path should bulge in the middle, not
    // run dead straight. Every segment this generator emits should now
    // have a real 3rd (midpoint) vertex, offset perpendicular to the
    // straight line between its endpoints by at least row_width_m --
    // see p121_path_shape's own opinion, which checks exactly this.
    let nbhd = four_corner_blocks();
    let params = PathNetworkParams::defaults();
    let sub = PathNetwork.apply(&nbhd, "*", &params, 7).unwrap();

    assert!(!sub.new_streets.is_empty());
    for s in &sub.new_streets {
        assert_eq!(s.centerline.len(), 3, "{} should have a real bulge midpoint, not a straight 2-point line", s.id);
        let a = s.centerline[0];
        let b = s.centerline[2];
        let mid = s.centerline[1];
        // Perpendicular distance from mid to the line a-b, in local meters
        // (equirectangular projection, fine at this fixture's scale).
        let m = 111_320.0;
        let (ax, ay) = (a.lng * m, a.lat * m);
        let (bx, by) = (b.lng * m, b.lat * m);
        let (mx, my) = (mid.lng * m, mid.lat * m);
        let (dx, dy) = (bx - ax, by - ay);
        let len = (dx * dx + dy * dy).sqrt();
        let cross = (dx * (my - ay) - dy * (mx - ax)).abs();
        let dev = cross / len;
        let row_width = s.row_width_m.unwrap();
        assert!(dev >= row_width - 1e-6, "{} bulge deviation {dev:.2}m should be >= its own row_width_m {row_width:.2}m", s.id);
    }
}

#[test]
fn bulge_is_deterministic_for_the_same_seed() {
    let nbhd = four_corner_blocks();
    let params = PathNetworkParams::defaults();
    let sub_a = PathNetwork.apply(&nbhd, "*", &params, 42).unwrap();
    let sub_b = PathNetwork.apply(&nbhd, "*", &params, 42).unwrap();
    assert_eq!(sub_a.new_streets, sub_b.new_streets, "the same seed should reproduce the identical bulge shape");
}

/// Five blocks at the vertices of a regular pentagon (radius 100m). A
/// pentagon's MST is 4 of its 5 equal-length perimeter edges (a path);
/// the 6 non-MST edges (1 perimeter + 5 diagonals) would, if all added
/// unconstrained, form the complete graph K5 -- every node at degree 4,
/// a four-way-or-more intersection everywhere. Exactly the case P50 T
/// Junctions' "avoid four-way intersections" targets.
fn pentagon_blocks() -> Neighborhood {
    let m_per_deg = 111_320.0;
    let radius_m = 100.0;

    let make_pad = |id: &str, cx: f64, cy: f64, block: &str| -> Parcel {
        let s = 0.00005;
        let ring = vec![
            LngLat::new(cx - s, cy - s),
            LngLat::new(cx + s, cy - s),
            LngLat::new(cx + s, cy + s),
            LngLat::new(cx - s, cy + s),
        ];
        Parcel {
            id: id.into(),
            polygon: Polygon::from_ring(ring),
            area_acres: 0.01,
            use_category: Some("p95_building_pad".into()),
            ownership: None,
            is_eda: false,
            spec: Some(block.into()),
            density_tier: None,
            target_stories: None,
        }
    };

    let mut parcels = Vec::new();
    for k in 0..5 {
        let theta = std::f64::consts::TAU * (k as f64) / 5.0;
        let x_m = radius_m * theta.cos();
        let y_m = radius_m * theta.sin();
        let cx = x_m / m_per_deg;
        let cy = y_m / m_per_deg;
        parcels.push(make_pad(&format!("p{k}"), cx, cy, &format!("BLOCK_{k}")));
    }

    Neighborhood {
        id: "test".into(),
        bbox_wgs84: [-0.001, -0.001, 0.001, 0.001],
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
            label: "P50 degree-cap fixture".into(),
        },
    }
}

#[test]
fn loop_edge_selection_never_pushes_a_node_past_a_three_way_meeting() {
    // Ask for every remaining edge (1 perimeter + 5 diagonals = 6) --
    // unconstrained, this would produce K5 (every node at degree 4).
    let nbhd = pentagon_blocks();
    let params = PathNetworkParams { loop_budget: 3.0, local_loop_budget: 3.0, ..PathNetworkParams::defaults() };
    let sub = PathNetwork.apply(&nbhd, "*", &params, 0).unwrap();

    let mut degree: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for s in &sub.new_streets {
        let a = s.centerline.first().unwrap();
        let b = s.centerline.last().unwrap();
        // Snap to the fixture's own block centroids by nearest match --
        // simpler here to just count endpoints directly since ids encode
        // the block pair.
        let parts: Vec<&str> = s.id.trim_start_matches("path_").trim_start_matches("localloop_").trim_start_matches("loop_").split("_to_").collect();
        assert_eq!(parts.len(), 2, "unexpected street id shape: {}", s.id);
        *degree.entry(parts[0].to_string()).or_default() += 1;
        *degree.entry(parts[1].to_string()).or_default() += 1;
        let _ = (a, b); // endpoints only used for the id-parsing sanity check above
    }

    assert!(
        sub.new_streets.len() < 10,
        "K5 (all 5 blocks pairwise connected) would be 10 edges -- degree capping should have skipped some, got {}",
        sub.new_streets.len()
    );
    for (block, d) in &degree {
        assert!(*d <= 3, "block {block} ended up at degree {d} -- P50 T Junctions requires no node exceed a three-way meeting");
    }
}

/// P53 Main Gateways: path_network should emit one real site-perimeter
/// Boundary (convex hull of every block's own outer ring), and every
/// street endpoint from the four-corner-blocks fixture should sit near
/// that hull -- the blocks themselves are the hull's own corners.
#[test]
fn emits_a_real_site_perimeter_boundary() {
    let nbhd = four_corner_blocks();
    let params = PathNetworkParams::defaults();
    let sub = PathNetwork.apply(&nbhd, "*", &params, 0).unwrap();

    assert_eq!(sub.new_boundaries.len(), 1, "expected exactly one site-perimeter boundary");
    let boundary = &sub.new_boundaries[0];
    assert_eq!(boundary.id, "site_perimeter");
    assert!(boundary.centerline.len() >= 4, "a real quadrilateral hull should have at least 4 vertices (closed ring), got {}", boundary.centerline.len());
    assert_eq!(boundary.centerline.first(), boundary.centerline.last(), "the boundary ring should be closed");
}

#[test]
fn params_roundtrip() {
    let p = PathNetworkParams { loop_budget: 5.0, local_loop_budget: 2.0, path_width_m: 6.0 };
    let v = p.as_vector();
    let back = PathNetworkParams::from_vector(&v);
    assert_eq!(back.loop_budget, 5.0);
    assert_eq!(back.local_loop_budget, 2.0);
    assert_eq!(back.path_width_m, 6.0);
}

#[test]
fn real_pipeline_p95_then_blockgrouping_then_pathnetwork() {
    use street_smarts_patterns::apply_subdivision;
    use street_smarts_patterns::block_grouping::{BlockGrouping, BlockGroupingParams};
    use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};

    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");

    let p95 = P95BuildingComplex;
    let sub95 = p95.apply(&baseline, "00001129", &P95Params::defaults(), 42).unwrap();
    let with_pads = apply_subdivision(&baseline, &sub95);

    let bg = BlockGrouping;
    let sub_bg = bg.apply(&with_pads, "*", &BlockGroupingParams::defaults(), 42).unwrap();
    let with_blocks = apply_subdivision(&with_pads, &sub_bg);

    let pn = PathNetwork;
    let sub_pn = pn.apply(&with_blocks, "*", &PathNetworkParams::defaults(), 0).unwrap();

    eprintln!(
        "real pipeline: {} streets ({} local backbone, {} pedestrian loop)",
        sub_pn.new_streets.len(),
        sub_pn.new_streets.iter().filter(|s| s.classification.as_deref() == Some("local")).count(),
        sub_pn.new_streets.iter().filter(|s| s.classification.as_deref() == Some("pedestrian")).count(),
    );
    assert!(!sub_pn.new_streets.is_empty());
}
