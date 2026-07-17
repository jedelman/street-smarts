//! The corrected pipeline, end to end, on real data: Alexander's own
//! pattern numbering (37 < 52 < 61 < 95 < 107) run in that order, instead
//! of the old pipeline's P95 -> BlockGrouping -> PathNetwork -> P107 -> P61.
//!
//! Sequence:
//!   1. P37 (once, site-scale): carve the raw parcel into BLOCK_n blocks.
//!   2. PathNetwork/P52 (once, site-scale): connect the blocks -- unchanged
//!      code, already filters by `spec.starts_with("BLOCK_")`, which P37
//!      produces directly.
//!   3. P61 (site-scale budget): a total of `max_squares` (default 4)
//!      spread across blocks by area (`pipeline::allocate_squares_by_area`)
//!      -- Alexander's "a few" public squares means a handful across the
//!      WHOLE site, not `max_squares` repeated on every block. Most blocks
//!      get zero. See p61's own module doc ("v0.6") for why the old
//!      per-block-full-budget version was the biggest single contributor to
//!      fragmentation.
//!   4. Per block: P95 (reworked) builds pads around whatever P61 placed on
//!      that block (if anything) plus street corridors from step 2.
//!   5. P107 (once, site-scale): shape every P95 pad for daylight depth --
//!      unchanged code, already filters by `use_category == "p95_building_pad"`
//!      across the whole neighborhood regardless of which block a pad came
//!      from.
//!
//! This is what a corrected `registry.rs`/web-UI pipeline should run; this
//! test is the real proof it actually composes end to end, not a mockup.
//! It reimplements `pipeline::run_corrected_pipeline`'s loop locally (rather
//! than calling it directly) so it can assert on intermediate per-stage
//! counts the shared function doesn't expose.

use street_smarts_core::nir::Neighborhood;
use street_smarts_patterns::p107_wings_of_light::{P107Params, P107WingsOfLight};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::p61_small_public_squares::{place_new_squares_n, P61Params, P61SmallPublicSquares};
use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};
use street_smarts_patterns::path_network::{PathNetwork, PathNetworkParams};
use street_smarts_patterns::pipeline::{allocate_squares_by_area, run_corrected_pipeline};
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

    // 3 + 4. P61's site-wide square budget (allocated by block area), then
    // per block: P95 (reworked, builds around whatever P61 placed).
    let block_areas: Vec<f64> = block_ids.iter()
        .map(|id| nbhd.parcels.iter().find(|p| &p.id == id).unwrap().polygon.area_m2())
        .collect();
    let total_squares = P61Params::defaults().max_squares.round().max(1.0) as usize;
    let square_counts = allocate_squares_by_area(&block_areas, total_squares);
    assert_eq!(
        square_counts.iter().sum::<usize>(), total_squares,
        "the site-wide square budget should be fully allocated across blocks, not per-block"
    );
    assert!(
        square_counts.iter().filter(|&&n| n == 0).count() > 0,
        "with only {total_squares} squares for {} blocks, most blocks should get none -- that's the whole point of allocating by area instead of stamping max_squares on every block",
        block_ids.len()
    );

    let mut n_blocks_with_squares = 0;
    let mut total_pads = 0;
    let mut total_courtyards = 0;
    let mut per_block_pad_counts: Vec<usize> = Vec::new();
    for (i, block_id) in block_ids.iter().enumerate() {
        let seed = 100 + i as u64;
        let n_squares = square_counts[i];
        if n_squares > 0 {
            let block_parcel = nbhd.parcels.iter().find(|p| &p.id == block_id).unwrap().clone();
            if let Ok(sub61) = place_new_squares_n(&nbhd, &block_parcel, n_squares, &P61Params::defaults(), seed, P61SmallPublicSquares.source()) {
                if !sub61.new_open_space.is_empty() {
                    n_blocks_with_squares += 1;
                }
                nbhd = apply_subdivision(&nbhd, &sub61);
            }
        }
        // P95 must still succeed even if a block got zero squares allocated
        // (falls through gracefully, builds on raw land minus street
        // corridors only).
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

/// Calls `pipeline::run_corrected_pipeline` directly -- the REAL shared
/// orchestration function, not a reimplemented loop -- against the real
/// MALL_CORE fixture, and checks the full interior-ontology sequence
/// (P127 -> P130 -> P129 -> P131 -> P221 -> P133) actually lands.
///
/// This is the regression test for a real bug this session: P133 was
/// first placed right after P131 (Alexander's own numbering), but
/// `Building.floors` isn't set until P221 runs (see `p133`'s own module
/// doc) -- every building's multi-story filter matched nothing, and the
/// pipeline's own `if let Ok(...)` silently swallowed the resulting
/// error. Every P133 UNIT test in `p133_staircase_as_a_stage.rs` hardcodes
/// `floors: Some(N)` directly into its fixture, so none of them could
/// have caught an ordering bug that only manifests when `floors` is
/// derived by an earlier stage the way the real pipeline actually does
/// it -- only a real, end-to-end run surfaces that.
#[test]
fn corrected_pipeline_places_real_interior_ontology_on_the_real_mall_parcel() {
    let raw = std::fs::read_to_string("../../data/eastside-proposal.json").expect("fixture present");
    let nbhd: Neighborhood = serde_json::from_str(&raw).expect("parseable");
    let mall = nbhd.parcels.iter().find(|p| p.spec.as_deref() == Some("MALL_CORE")).expect("MALL_CORE in proposal fixture");

    let result = run_corrected_pipeline(&nbhd, &mall.id, 42);
    assert!(!result.buildings.is_empty(), "pipeline should have produced real buildings");

    let n_buildings = result.buildings.len();
    let n_with_entrance = result.buildings.iter().filter(|b| b.interior_cells.iter().any(|c| c.kind == "entrance")).count();
    let n_with_common = result.buildings.iter().filter(|b| b.interior_cells.iter().any(|c| c.is_common)).count();
    let n_multi_story = result.buildings.iter().filter(|b| b.floors.unwrap_or(1) >= 2).count();
    let n_with_stair = result.buildings.iter().filter(|b| b.interior_cells.iter().any(|c| c.kind == "stair")).count();

    eprintln!(
        "Corrected pipeline interior ontology on real MALL_CORE: {n_buildings} buildings, \
         {n_with_entrance} with an entrance cell, {n_with_common} with a common-area cell, \
         {n_multi_story} multi-story, {n_with_stair} with a stair core."
    );

    assert_eq!(n_with_entrance, n_buildings, "P130 should tag an entrance cell on every building");
    assert_eq!(n_with_common, n_buildings, "P129 should mark a common-area cell on every building");
    assert!(n_multi_story > 0, "P221 should have derived floors >= 2 for at least one real building on this site");
    assert_eq!(
        n_with_stair, n_multi_story,
        "P133 should place a real stair core in every multi-story building with a common area -- \
         if this is 0 while n_multi_story > 0, P133 almost certainly regressed back to running \
         before Building.floors is set (see this test's own doc comment)"
    );
}
