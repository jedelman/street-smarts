//! Proof, not just a unit test: the pattern_order_prototype experiment
//! measured a REAL 553 m² of overlap when P95 ran unmodified next to
//! pre-placed P52/P61 geometry. This test does the same setup, but with
//! P95's rework (reserved-land subtraction, see p95_building_complex.rs's
//! `reserved_holes_for_part`) actually wired in -- and asserts the overlap
//! is now zero.

use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{Neighborhood, OpenSpace, OpenSpaceKind, Street};
use street_smarts_patterns::p95_building_complex::{stratified_seeds, P95BuildingComplex, P95Params};
use street_smarts_patterns::planar::{
    area, clip_to_polygon, clip_to_polygon_largest, kruskal_mst, local_to_lnglat, local_to_ring,
    ring_to_local, MstResult, Pt2,
};
use street_smarts_patterns::prng::Prng;
use street_smarts_patterns::{apply_subdivision, Parameters, PatternOperator};
use street_smarts_patterns::subdivision::Subdivision;

#[test]
fn p95_builds_pads_around_pre_placed_squares_with_zero_overlap() {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");

    let parcel = baseline
        .parcels
        .iter()
        .find(|p| p.id == "00001129")
        .expect("mall parcel present in baseline fixture");

    let origin = LngLat::new(
        parcel.polygon.outer.iter().map(|p| p.lng).sum::<f64>() / parcel.polygon.outer.len() as f64,
        parcel.polygon.outer.iter().map(|p| p.lat).sum::<f64>() / parcel.polygon.outer.len() as f64,
    );
    let local_parcel = ring_to_local(&parcel.polygon.outer, &origin);
    assert!(local_parcel.len() >= 3);

    // === Same P52 + P61 -- first, on raw land -- setup as the prototype. ===
    let mut prng = Prng::new(42);
    let path_nodes = stratified_seeds(&local_parcel, 6, 0.4, &mut prng);
    assert!(path_nodes.len() >= 3);

    let MstResult { mst_edges, .. } = kruskal_mst(&path_nodes);
    let mut new_streets: Vec<Street> = Vec::new();
    for (i, j, _d) in &mst_edges {
        new_streets.push(Street {
            id: format!("proto_p52_link_{i}_{j}"),
            centerline: vec![local_to_lnglat(path_nodes[*i], &origin), local_to_lnglat(path_nodes[*j], &origin)],
            classification: Some("local".into()),
            row_width_m: Some(4.0),
            surface: None,
        });
    }

    let square_half_side = 18.3 / 2.0;
    let n_squares = (path_nodes.len() / 2).clamp(1, 4);
    let mut new_open: Vec<OpenSpace> = Vec::new();
    for (idx, &node) in path_nodes.iter().take(n_squares).enumerate() {
        let square_local = vec![
            Pt2::new(node.x - square_half_side, node.y - square_half_side),
            Pt2::new(node.x + square_half_side, node.y - square_half_side),
            Pt2::new(node.x + square_half_side, node.y + square_half_side),
            Pt2::new(node.x - square_half_side, node.y + square_half_side),
        ];
        let clipped = clip_to_polygon_largest(&square_local, &local_parcel);
        if clipped.len() < 3 || area(&clipped) < 20.0 {
            continue;
        }
        new_open.push(OpenSpace {
            id: format!("proto_p61_sq{idx}"),
            polygon: Polygon::from_ring(local_to_ring(&clipped, &origin)),
            kind: OpenSpaceKind::Plaza,
        });
    }
    assert!(!new_open.is_empty(), "should place at least one real square directly");
    let squares_total_area: f64 = new_open.iter().map(|o| o.polygon.area_m2()).sum();

    // Merge the pre-placed P52/P61 geometry into the neighborhood -- this is
    // what a corrected pipeline would have already done by the time P95 runs.
    let proto_sub = Subdivision {
        new_parcels: vec![],
        new_open_space: new_open.clone(),
        new_buildings: vec![],
        new_streets: new_streets.clone(),
        replaced_parcel_ids: vec![],
        replaced_open_space_ids: vec![],
        replaced_building_ids: vec![],
        entity_provenance: std::collections::BTreeMap::new(),
        trace: street_smarts_patterns::SubdivisionTrace {
            operator_name: "proto_p52_p61".into(),
            operator_source: street_smarts_core::opinion::SourceCitation {
                id: "proto".into(), display: "prototype".into(), url: None,
            },
            headline: "prototype P52+P61 placement".into(),
            steps: vec![],
            caveats: vec![],
            seed: 42,
            params: serde_json::Value::Null,
        },
    };
    let with_reserved_land = apply_subdivision(&baseline, &proto_sub);
    assert_eq!(with_reserved_land.open_space.len(), new_open.len());
    assert_eq!(with_reserved_land.streets.len(), new_streets.len());

    // === P95, REWORKED: run on the neighborhood that now has real
    // reserved land on it. ===
    let p95 = P95BuildingComplex;
    let sub95 = p95
        .apply(&with_reserved_land, "00001129", &P95Params::defaults(), 42)
        .expect("P95 should still produce pads around the reserved land");

    assert!(sub95.trace.steps.iter().any(|s| s.contains("reserved hole")), "trace should mention subtracting reserved land, got: {:?}", sub95.trace.steps);
    eprintln!("--- P95 trace steps ---");
    for s in &sub95.trace.steps {
        eprintln!("  {s}");
    }

    // The real proof: zero actual polygon overlap between P95's new pads
    // (and its own courtyard) and the pre-placed squares.
    let mut overlap_area = 0.0;
    let mut overlap_features = 0;
    let labeled_features = sub95.new_parcels.iter().map(|p| (p.id.as_str(), &p.polygon))
        .chain(sub95.new_open_space.iter().map(|o| (o.id.as_str(), &o.polygon)));
    for (fid, p) in labeled_features {
        let local_feature = ring_to_local(&p.outer, &origin);
        for square in &new_open {
            let local_square = ring_to_local(&square.polygon.outer, &origin);
            let pieces = clip_to_polygon(&local_square, &local_feature);
            let piece_area: f64 = pieces.iter().map(|piece| area(piece)).sum();
            if piece_area > 0.5 {
                eprintln!("OVERLAP: feature {fid} overlaps square {} by {piece_area:.2} m²", square.id);
                overlap_area += piece_area;
                overlap_features += 1;
            }
        }
    }

    eprintln!("=== P95 rework: real overlap check against pre-placed P52/P61 land ===");
    eprintln!("Pre-placed squares: {} ({:.0} m² total)", new_open.len(), squares_total_area);
    eprintln!("P95 (reworked) output: {} pad(s) + {} courtyard(s)", sub95.new_parcels.len(), sub95.new_open_space.len());
    eprintln!("Real overlap area between P95's output and the pre-placed squares: {:.1} m² across {} feature(s) (prototype's UNMODIFIED P95 measured 553 m² of real overlap in the same setup).", overlap_area, overlap_features);

    assert!(overlap_area < 1.0, "reworked P95 should produce zero real overlap with pre-placed land, got {overlap_area} m²");
    assert_eq!(overlap_features, 0, "no P95 feature should overlap a pre-placed square, got {overlap_features}");
}
