//! The corrected pipeline, end to end, on real data: Alexander's own
//! pattern numbering (37 < 52 < 61 < 95 < 107) run in that order, instead
//! of the old pipeline's P95 -> BlockGrouping -> PathNetwork -> P107 -> P61.
//!
//! Sequence:
//!   1. P37 (once, site-scale): carve the raw parcel into BLOCK_n blocks.
//!   2. PathNetwork/P52 (once, site-scale): connect the blocks -- unchanged
//!      code, already filters by `spec.starts_with("BLOCK_")`, which P37
//!      produces directly.
//!   3. Per block: P61 places a few squares on the block's raw land, then
//!      P95 (reworked) builds pads around them.
//!   4. P107 (once, site-scale): shape every P95 pad for daylight depth --
//!      unchanged code, already filters by `use_category == "p95_building_pad"`
//!      across the whole neighborhood regardless of which block a pad came
//!      from.
//!
//! This is what a corrected `registry.rs`/web-UI pipeline should run; this
//! test is the real proof it actually composes end to end, not a mockup.

use street_smarts_core::nir::Neighborhood;
use street_smarts_patterns::p107_wings_of_light::{P107Params, P107WingsOfLight};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::p61_small_public_squares::{P61Params, P61SmallPublicSquares};
use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};
use street_smarts_patterns::path_network::{PathNetwork, PathNetworkParams};
use street_smarts_patterns::{apply_subdivision, Parameters, PatternOperator};

#[test]
fn corrected_pipeline_runs_end_to_end_on_the_real_mall_parcel() {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");

    // 1. P37, once, site-scale.
    let sub37 = P37HouseCluster.apply(&baseline, "00001129", &P37Params::defaults(), 42)
        .expect("P37 should carve the raw parcel into blocks");
    let mut nbhd = apply_subdivision(&baseline, &sub37);
    let block_ids: Vec<String> = nbhd.parcels.iter()
        .filter(|p| p.spec.as_deref().unwrap_or("").starts_with("BLOCK_"))
        .map(|p| p.id.clone())
        .collect();
    assert!(block_ids.len() >= 5, "should have several real blocks, got {}", block_ids.len());

    // 2. PathNetwork/P52, once, site-scale -- connects the blocks P37 just made.
    let sub52 = PathNetwork.apply(&nbhd, "*", &PathNetworkParams::defaults(), 42)
        .expect("PathNetwork should connect P37's blocks with zero changes on its end");
    nbhd = apply_subdivision(&nbhd, &sub52);
    assert!(!nbhd.streets.is_empty(), "blocks should be connected by real streets");

    // 3. Per block: P61 (raw-land placement) then P95 (reworked, builds around it).
    let mut n_blocks_with_squares = 0;
    let mut total_pads = 0;
    let mut total_courtyards = 0;
    let mut per_block_pad_counts: Vec<usize> = Vec::new();
    for (i, block_id) in block_ids.iter().enumerate() {
        let seed = 100 + i as u64;
        if let Ok(sub61) = P61SmallPublicSquares.apply(&nbhd, block_id, &P61Params::defaults(), seed) {
            if !sub61.new_open_space.is_empty() {
                n_blocks_with_squares += 1;
            }
            nbhd = apply_subdivision(&nbhd, &sub61);
        }
        // P95 must still succeed even if P61 found the block too small/
        // concave to place a square on (falls through gracefully).
        if let Ok(sub95) = P95BuildingComplex.apply(&nbhd, block_id, &P95Params::defaults(), seed) {
            total_pads += sub95.new_parcels.len();
            total_courtyards += sub95.new_open_space.len();
            per_block_pad_counts.push(sub95.new_parcels.len());
            nbhd = apply_subdivision(&nbhd, &sub95);
        }
    }
    assert!(n_blocks_with_squares > 0, "at least one block should have gotten real squares placed on it");
    assert!(total_pads > 0, "P95 should have produced real building pads across the blocks");

    // 4. P107, once, site-scale -- shapes every pad regardless of which block it came from.
    let sub107 = P107WingsOfLight.apply(&nbhd, "*", &P107Params::defaults(), 42)
        .expect("P107 should shape pads across all blocks in one pass");
    nbhd = apply_subdivision(&nbhd, &sub107);
    assert!(!nbhd.buildings.is_empty(), "P107 should have produced real building geometry");

    // The actual point of this whole session: no single block/courtyard
    // should look anything like the old flat-mesh result (99 pads, one
    // block "containing" nearly the whole 47-acre site). Real hierarchy:
    // multiple blocks, each with a human-scaled pad count.
    let max_pads_in_one_block = per_block_pad_counts.iter().cloned().max().unwrap_or(0);
    eprintln!(
        "Corrected pipeline on real mall parcel: {} blocks, {} with squares placed, {} total pads across blocks (max {} in any one block), {} total courtyards, {} buildings shaped.",
        block_ids.len(), n_blocks_with_squares, total_pads, max_pads_in_one_block, total_courtyards, nbhd.buildings.len()
    );
    assert!(
        max_pads_in_one_block < 30,
        "no single block should have anywhere near the old flat mesh's ~100 pads -- real hierarchy means each block stays human-scaled, got {max_pads_in_one_block}"
    );
}
