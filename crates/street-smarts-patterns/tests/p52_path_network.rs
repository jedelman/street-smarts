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
    let params = PathNetworkParams { loop_budget: 0.0, ..PathNetworkParams::defaults() };
    let sub = PathNetwork.apply(&nbhd, "*", &params, 0).unwrap();

    // 4 blocks -> MST has exactly 3 edges, no loop edges.
    assert_eq!(sub.new_streets.len(), 3, "pure tree should have n-1 = 3 edges for 4 blocks");
    assert!(
        sub.new_streets.iter().all(|s| s.classification.as_deref() == Some("local")),
        "with loop_budget=0, every edge should be MST backbone ('local')"
    );
    // A square's MST is 3 sides, never a diagonal -- diagonals are longer.
    assert!(
        sub.new_streets.iter().all(|s| !s.id.starts_with("loop_")),
        "no loop-prefixed edges should exist when loop_budget=0"
    );
}

#[test]
fn loop_budget_one_adds_exactly_one_pedestrian_edge() {
    let nbhd = four_corner_blocks();
    let params = PathNetworkParams { loop_budget: 1.0, ..PathNetworkParams::defaults() };
    let sub = PathNetwork.apply(&nbhd, "*", &params, 0).unwrap();

    assert_eq!(sub.new_streets.len(), 4, "MST (3) + loop_budget 1 = 4 edges");
    let local = sub.new_streets.iter().filter(|s| s.classification.as_deref() == Some("local")).count();
    let pedestrian = sub.new_streets.iter().filter(|s| s.classification.as_deref() == Some("pedestrian")).count();
    assert_eq!(local, 3, "MST backbone should still be exactly 3 edges");
    assert_eq!(pedestrian, 1, "loop_budget=1 should add exactly one supplementary edge");
}

#[test]
fn loop_budget_never_exceeds_available_non_mst_edges() {
    // 4 blocks -> complete graph has 6 edges, MST takes 3, leaving 3
    // possible loop edges. Asking for 10 shouldn't crash or duplicate --
    // it should just take all 3 available.
    let nbhd = four_corner_blocks();
    let params = PathNetworkParams { loop_budget: 10.0, ..PathNetworkParams::defaults() };
    let sub = PathNetwork.apply(&nbhd, "*", &params, 0).unwrap();
    assert_eq!(sub.new_streets.len(), 6, "should cap at the complete graph, not error or duplicate");
}

#[test]
fn params_roundtrip() {
    let p = PathNetworkParams { loop_budget: 5.0, path_width_m: 6.0 };
    let v = p.as_vector();
    let back = PathNetworkParams::from_vector(&v);
    assert_eq!(back.loop_budget, 5.0);
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
