//! The real corrected pipeline (see
//! `street_smarts_patterns::pipeline::run_corrected_pipeline_with_p37_traced`),
//! run through a `HistoryStore` instead of direct `.apply()` calls, so
//! every commit is real, cached, and replayable -- not just narrated in a
//! trace string.
//!
//! This is the single source of truth `examples/dump_pipeline.rs` (which
//! only needs the final state) and `examples/dump_lineage_animation.rs`
//! (which needs every intermediate commit) both build on now, instead of
//! each independently computing the same 16-stage pipeline and risking
//! the two silently drifting apart -- exactly the kind of duplicated-
//! source-of-truth bug this codebase has caught and fixed before (see
//! `language_graph.rs`'s own self-verifying test against this same
//! pipeline's real trace, and P29's `from_label`/`from_ring` dual-path
//! property test).
//!
//! Mirrors `run_corrected_pipeline_with_p37_traced` exactly: same 16
//! stages, same targets, same per-block P61 area-budget split, same
//! skip-tolerance (`if let Ok`, not an abort).

use crate::{Commit, HistoryStore, NeighborhoodId};
use street_smarts_core::Scope;
use street_smarts_patterns::p107_wings_of_light::{P107Params, P107WingsOfLight};
use street_smarts_patterns::p108_connected_buildings::{P108ConnectedBuildings, P108Params};
use street_smarts_patterns::p124_activity_pockets::{P124ActivityPockets, P124Params};
use street_smarts_patterns::p117_sheltering_roof::{P117Params, P117ShelteringRoof};
use street_smarts_patterns::p127_intimacy_gradient::{P127IntimacyGradient, P127Params};
use street_smarts_patterns::p129_common_areas_at_the_heart::{P129CommonAreasAtTheHeart, P129Params};
use street_smarts_patterns::p130_entrance_room::{P130EntranceRoom, P130Params};
use street_smarts_patterns::p131_the_flow_through_rooms::{P131Params, P131TheFlowThroughRooms};
use street_smarts_patterns::p133_staircase_as_a_stage::{P133Params, P133StaircaseAsAStage};
use street_smarts_patterns::p197_thick_walls::{P197Params, P197ThickWalls};
use street_smarts_patterns::p221_natural_doors_and_windows::{P221NaturalDoorsAndWindows, P221Params};
use street_smarts_patterns::p29_density_rings::{P29DensityRings, P29Params};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::p61_small_public_squares::{P61Params, P61SmallPublicSquares};
use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};
use street_smarts_patterns::p96_number_of_stories::{P96NumberOfStories, P96Params};
use street_smarts_patterns::path_network::{PathNetwork, PathNetworkParams};
use street_smarts_patterns::pipeline::allocate_squares_by_area;
use street_smarts_patterns::{DynOperator, Parameters};

/// Try `op` at `target`/`params`/`seed` against `*cur`; on success, record
/// the real `Commit` and advance `*cur`. On failure (a real, expected skip
/// -- a block too small, an operator with nothing left to do), leaves
/// everything untouched, matching `pipeline.rs`'s own `if let Ok(...)`
/// tolerance.
#[allow(clippy::too_many_arguments)]
fn try_run(
    store: &mut dyn HistoryStore,
    op: &dyn DynOperator,
    target: &str,
    params: &serde_json::Value,
    seed: u64,
    cur: &mut NeighborhoodId,
    commits: &mut Vec<Commit>,
) {
    if let Ok(next) = store.get_or_compute(*cur, op, target, params, seed, "v1") {
        if let Some(c) = store.commit(&next) {
            commits.push(c);
        }
        *cur = next;
    }
}

/// Runs the real 16-stage corrected pipeline against `root` via
/// `store.get_or_compute`, returning the final commit id plus every real
/// commit that succeeded, in order (empty list entries are never
/// inserted -- a skipped stage just doesn't appear).
pub fn run_corrected_pipeline_via_ledger(
    store: &mut dyn HistoryStore,
    root: NeighborhoodId,
    parcel_id: &str,
    seed: u64,
) -> (NeighborhoodId, Vec<Commit>) {
    let mut cur = root;
    let mut commits: Vec<Commit> = Vec::new();

    try_run(store, &P37HouseCluster, parcel_id, &P37Params::defaults().as_map(), seed, &mut cur, &mut commits);
    try_run(store, &PathNetwork, "*", &PathNetworkParams::defaults().as_map(), seed, &mut cur, &mut commits);
    try_run(store, &P29DensityRings, "*", &P29Params::defaults().as_map(), seed, &mut cur, &mut commits);

    // Site-scale square budget split across blocks by area -- same
    // computation `pipeline.rs` itself uses, via its own real `pub fn`
    // rather than a second, independently-maintained copy.
    let after_p29 = store.materialize(&cur).unwrap_or_else(|e| panic!("commit {cur:?} must materialize: {e}"));
    let block_ids: Vec<String> = after_p29.select_ids(&Scope::Block);
    let block_areas: Vec<f64> = block_ids.iter()
        .map(|id| after_p29.parcels.iter().find(|p| &p.id == id).map(|p| p.polygon.area_m2()).unwrap_or(0.0))
        .collect();
    let total_squares = P61Params::defaults().max_squares.round().max(1.0) as usize;
    let square_counts = allocate_squares_by_area(&block_areas, total_squares);

    for (i, block_id) in block_ids.iter().enumerate() {
        let block_seed = seed + i as u64 + 1;
        let n_squares = square_counts[i];
        if n_squares > 0 {
            // `P61SmallPublicSquares::apply` with no existing Plaza on
            // this block falls through to the same `place_new_squares_n`
            // logic `pipeline.rs` calls directly, given the SAME target
            // square count via `max_squares` -- see
            // `p61_small_public_squares.rs`'s own `place_new_squares`
            // thin wrapper.
            let p61_params = P61Params { max_squares: n_squares as f64, ..P61Params::defaults() };
            try_run(store, &P61SmallPublicSquares, block_id, &p61_params.as_map(), block_seed, &mut cur, &mut commits);
        }
        try_run(store, &P95BuildingComplex, block_id, &P95Params::defaults().as_map(), block_seed, &mut cur, &mut commits);
    }

    try_run(store, &P108ConnectedBuildings, "*", &P108Params::defaults().as_map(), seed, &mut cur, &mut commits);
    try_run(store, &P96NumberOfStories, "*", &P96Params::defaults().as_map(), seed, &mut cur, &mut commits);
    try_run(store, &P107WingsOfLight, "*", &P107Params::defaults().as_map(), seed, &mut cur, &mut commits);
    // Right after P107, strictly before P197/P127/P221 -- all of which
    // need the FINAL building footprint. See pipeline.rs's own step 9 doc.
    try_run(store, &P124ActivityPockets, "*", &P124Params::defaults().as_map(), seed, &mut cur, &mut commits);
    try_run(store, &P117ShelteringRoof, "*", &P117Params::defaults().as_map(), seed, &mut cur, &mut commits);
    // Right after P107/P124 -- every downstream stage clones-and-mutates
    // the buildings P107/P124 produced, so wall_thickness_m survives
    // untouched. See pipeline.rs's own step 10 doc.
    try_run(store, &P197ThickWalls, "*", &P197Params::defaults().as_map(), seed, &mut cur, &mut commits);
    try_run(store, &P127IntimacyGradient, "*", &P127Params::defaults().as_map(), seed, &mut cur, &mut commits);
    try_run(store, &P130EntranceRoom, "*", &P130Params::defaults().as_map(), seed, &mut cur, &mut commits);
    try_run(store, &P129CommonAreasAtTheHeart, "*", &P129Params::defaults().as_map(), seed, &mut cur, &mut commits);
    try_run(store, &P131TheFlowThroughRooms, "*", &P131Params::defaults().as_map(), seed, &mut cur, &mut commits);
    try_run(store, &P221NaturalDoorsAndWindows, "*", &P221Params::defaults().as_map(), seed, &mut cur, &mut commits);
    // AFTER P221, not right after P131 -- Building.floors isn't set until
    // P221 derives it from real height. See pipeline.rs's own step 14 doc.
    try_run(store, &P133StaircaseAsAStage, "*", &P133Params::defaults().as_map(), seed, &mut cur, &mut commits);

    (cur, commits)
}
