//! End-to-end pipeline: P95 → BlockGrouping → PathNetwork → BuildingShape
//! against the real MALL_CORE.

use serde_json::Value;
use std::fs;
use street_smarts_core::nir::Neighborhood;
use street_smarts_patterns::{apply_subdivision, run_operator};

#[test]
fn pipeline_mall_core_v01() {
    let raw = fs::read_to_string("../../data/eastside-proposal.json").expect("fixture");
    let nbhd: Neighborhood = serde_json::from_str(&raw).expect("parseable");
    let mall = nbhd.parcels.iter().find(|p| p.spec.as_deref() == Some("MALL_CORE")).unwrap();
    let seed = 42u64;

    // Step 1: P95 — parcel → pads
    let p95 = run_operator(&nbhd, "p95_building_complex", &mall.id, &Value::Null, seed)
        .expect("p95");
    let after_p95 = apply_subdivision(&nbhd, &p95);
    eprintln!("after P95: {} parcels, {} pads, {} open_space",
        after_p95.parcels.len(),
        after_p95.parcels.iter().filter(|p| p.use_category.as_deref() == Some("p95_building_pad")).count(),
        after_p95.open_space.len(),
    );

    // Step 2: BlockGrouping — pads → blocks
    let blocks = run_operator(&after_p95, "block_grouping", "*", &Value::Null, seed)
        .expect("block_grouping");
    let after_blocks = apply_subdivision(&after_p95, &blocks);
    let block_count: std::collections::HashSet<_> = after_blocks.parcels.iter()
        .filter_map(|p| p.spec.as_ref().filter(|s| s.starts_with("BLOCK_")))
        .collect();
    eprintln!("after BlockGrouping: {} unique blocks", block_count.len());

    // Step 3: PathNetwork — blocks → streets
    let paths = run_operator(&after_blocks, "path_network", "*", &Value::Null, seed)
        .expect("path_network");
    let after_paths = apply_subdivision(&after_blocks, &paths);
    eprintln!("after PathNetwork: {} streets", after_paths.streets.len());

    // Step 4: BuildingShape — pads → buildings
    let shapes = run_operator(&after_paths, "building_shape", "*", &Value::Null, seed)
        .expect("building_shape");
    let after_shapes = apply_subdivision(&after_paths, &shapes);
    eprintln!("after BuildingShape: {} buildings", after_shapes.buildings.len());

    assert!(after_shapes.parcels.len() > nbhd.parcels.len(),
        "pipeline should net more parcels than source");
    assert!(after_shapes.buildings.len() >= 5, "expected several buildings");
    assert!(after_shapes.streets.len() >= 1, "expected at least one path");
    assert!(block_count.len() >= 2, "expected at least 2 blocks");
}
