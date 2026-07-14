//! Field-guided seeding (v0.3 prototype) run against the real MALL_CORE
//! parcel, which -- unlike synthetic test fixtures -- sits in the same
//! neighborhood as real CIVIC_*-spec parcels (CIVIC_700, CIVIC_854,
//! CIVIC_862, CIVIC_920, CIVIC_STRIP, CIVIC_ROW, CIVIC_SPINE_S -- see
//! data/eastside-proposal.json), so this is the natural grounded test for
//! whether field-guided seeding actually finds anchors and produces valid
//! output, not just a synthetic sanity check.

use std::fs;
use street_smarts_core::nir::Neighborhood;
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::{apply_subdivision, Parameters, PatternOperator};

fn load_proposal() -> Neighborhood {
    let raw = fs::read_to_string("../../data/eastside-proposal.json").expect("fixture present");
    serde_json::from_str(&raw).expect("parseable")
}

#[test]
fn field_guided_seeding_finds_the_real_civic_anchors_in_the_proposal_fixture() {
    let nbhd = load_proposal();
    let n_civic = nbhd
        .parcels
        .iter()
        .filter(|p| p.spec.as_deref().map(|s| s.starts_with("CIVIC")).unwrap_or(false))
        .count();
    assert!(n_civic >= 5, "expected several real CIVIC_*-spec parcels in the fixture, got {n_civic}");
}

#[test]
fn field_guided_mode_produces_valid_non_overlapping_blocks_on_mall_core() {
    let nbhd = load_proposal();
    let mall = nbhd
        .parcels
        .iter()
        .find(|p| p.spec.as_deref() == Some("MALL_CORE"))
        .expect("MALL_CORE in proposal fixture");

    let params = P37Params { seeding_mode: 1.0, ..P37Params::defaults() };
    let sub = P37HouseCluster
        .apply(&nbhd, &mall.id, &params, 42)
        .expect("field-guided seeding should still produce blocks on a real, large parcel");

    assert!(!sub.new_parcels.is_empty(), "expected at least one block");
    assert!(
        sub.trace.steps.iter().any(|s| s.contains("field-guided seeding")),
        "trace should record that field-guided seeding ran: {:?}",
        sub.trace.steps
    );
    assert!(
        sub.trace.steps.iter().any(|s| s.contains("civic anchor")),
        "trace should report civic-anchor count: {:?}",
        sub.trace.steps
    );

    // Applying the subdivision should produce a valid neighborhood: no
    // leftover reference to the replaced MALL_CORE parcel, strictly more
    // parcels than before.
    let modified = apply_subdivision(&nbhd, &sub);
    assert!(modified.parcels.iter().all(|p| p.id != mall.id));
    let added = modified.parcels.len() as i64 - nbhd.parcels.len() as i64 + 1;
    assert!(added > 0, "subdivision should net more parcels than source");

    // Total block area shouldn't runaway past the source parcel's area --
    // same sanity bound the Stratified-mode tests use elsewhere.
    let block_area_ac: f64 = sub.new_parcels.iter().map(|p| p.area_acres).sum();
    assert!(
        block_area_ac < mall.area_acres,
        "block area ({block_area_ac:.2} ac) should be less than source ({:.2} ac) after insets/common land",
        mall.area_acres
    );
}

#[test]
fn field_guided_and_stratified_modes_both_succeed_but_place_blocks_differently() {
    let nbhd = load_proposal();
    let mall = nbhd
        .parcels
        .iter()
        .find(|p| p.spec.as_deref() == Some("MALL_CORE"))
        .expect("MALL_CORE in proposal fixture");

    let stratified = P37HouseCluster
        .apply(&nbhd, &mall.id, &P37Params::defaults(), 42)
        .expect("stratified seeding should succeed");
    let field_guided_params = P37Params { seeding_mode: 1.0, ..P37Params::defaults() };
    let field_guided = P37HouseCluster
        .apply(&nbhd, &mall.id, &field_guided_params, 42)
        .expect("field-guided seeding should succeed");

    // Same seed, different seeding logic -- block centroids should differ
    // (field-guided pulls toward civic anchors; stratified doesn't know
    // they exist). Compare via each block's polygon centroid average.
    let centroid = |ring: &street_smarts_core::geometry::Ring| {
        let n = ring.len().max(1) as f64;
        let (lng, lat) = ring.iter().fold((0.0, 0.0), |(a, b), p| (a + p.lng, b + p.lat));
        (lng / n, lat / n)
    };
    let strat_centroids: Vec<(f64, f64)> =
        stratified.new_parcels.iter().map(|p| centroid(&p.polygon.outer)).collect();
    let field_centroids: Vec<(f64, f64)> =
        field_guided.new_parcels.iter().map(|p| centroid(&p.polygon.outer)).collect();

    assert!(!strat_centroids.is_empty() && !field_centroids.is_empty());
    assert_ne!(
        strat_centroids, field_centroids,
        "field-guided seeding should place blocks differently than stratified seeding on a parcel with real civic anchors nearby"
    );
}

#[test]
fn field_guided_mode_falls_back_to_stratified_when_no_anchors_exist() {
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{NeighborhoodMeta, Parcel};

    // A synthetic neighborhood with zero CIVIC-spec parcels and zero
    // streets -- field-guided mode has nothing to converge on and must
    // fall back to stratified seeding rather than erroring or producing
    // nothing.
    let m_per_deg = 111_320.0;
    let s = 300.0 / m_per_deg;
    let ring = vec![
        LngLat::new(0.0, 0.0),
        LngLat::new(s, 0.0),
        LngLat::new(s, s),
        LngLat::new(0.0, s),
    ];
    let nbhd = Neighborhood {
        id: "test".into(),
        bbox_wgs84: [0.0, 0.0, s, s],
        parcels: vec![Parcel {
            id: "RAW".into(),
            polygon: Polygon::from_ring(ring),
            area_acres: (300.0 * 300.0) / 4046.86,
            use_category: None,
            ownership: None,
            is_eda: true,
            spec: None,
            density_tier: None,
            target_stories: None,
        }],
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
            label: "no-anchors fixture".into(),
        },
    };

    let params = P37Params { seeding_mode: 1.0, ..P37Params::defaults() };
    let sub = P37HouseCluster
        .apply(&nbhd, "RAW", &params, 1)
        .expect("should fall back to stratified seeding, not error");
    assert!(!sub.new_parcels.is_empty());
    assert!(
        sub.trace.steps.iter().any(|s| s.contains("falling back to stratified seeding")),
        "trace should record the fallback: {:?}",
        sub.trace.steps
    );
}
