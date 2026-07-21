//! The corrected pipeline, end to end: Alexander's own pattern numbering
//! (37 < 52 < 61 < 95 < 107) run in that order, instead of the old
//! pipeline's P95 -> BlockGrouping -> PathNetwork -> P107 -> P61.
//!
//! Sequence:
//!   1. P37 (once, site-scale): carve the raw parcel into BLOCK_n blocks,
//!      each with an informal `OpenSpaceKind::Common` patch reserved for
//!      that cluster's own shared land (see p37's module doc).
//!   2. PathNetwork/P52 (once, site-scale): connect the blocks -- unchanged
//!      code, already filters by `spec.starts_with("BLOCK_")`, which P37
//!      produces directly.
//!   3. P29 Density Rings (once, site-scale): tags each BLOCK_n with a
//!      density tier and target story count from its distance to the
//!      site's own density center -- see p29's module doc for why it runs
//!      here instead of at its real Alexander-numbered position (29, well
//!      before House Cluster).
//!   4. P61 (site-scale budget, spread across blocks): Alexander's "a few"
//!      public squares means a handful across the WHOLE site, not
//!      `max_squares` repeated on every block -- see p61's own module doc,
//!      "v0.6" section, for why that was the biggest single contributor to
//!      block-level fragmentation. `allocate_squares_by_area` splits the
//!      site's `max_squares` budget across blocks proportional to block
//!      area (largest-remainder rounding); most blocks get zero. Squares
//!      are seeded on whatever land P37's common land didn't already claim
//!      on that block (`place_new_squares_n` subtracts existing reserved
//!      open space before seeding).
//!   5. Per block: P95 (reworked) builds pads around whatever P37/P61
//!      placed on that block, plus street corridors from step 2 -- each pad
//!      inherits its source block's P29 density tier/target. `pad_inset_m`
//!      is now a construction-joint-sized 0.1m, not a real setback.
//!   6. P108 Connected Buildings (once, site-scale): merges pads separated
//!      by nothing but that construction joint into one continuous
//!      party-wall footprint -- pads separated by a real reserved gap
//!      (street, square, common land) stay apart. Runs before P96/P107 so
//!      daylight-depth shaping sees the real, final connected mass; see
//!      p108's own module doc for why that deviates from Alexander's
//!      numbering (108, after Wings of Light).
//!   7. P96 Number of Stories (once, site-scale): turns each pad's
//!      inherited tier target into a real per-pad story count, capping
//!      ordinary buildings at 4 stories (P21 Four-Story Limit) with a very
//!      few, widely-spaced exceptions where a tier's target calls for more.
//!   8. P107 (once, site-scale): shape every P95/P108 pad for daylight
//!      depth, reading P96's `target_stories` assignment for real height --
//!      unless P96 didn't run, the flat `assumed_height_m` fallback applies
//!      exactly as before P96 existed. Already filters by
//!      `use_category == "p95_building_pad"` across the whole neighborhood
//!      regardless of which block a pad came from, and no longer applies
//!      its own setback on top of a P95/P108 pad's own gap (see p107's
//!      "v0.2" module doc).
//!   9. P124 Activity Pockets (once, site-scale): bumps a small, real,
//!      partly enclosed pocket out from up to max_pockets_per_plaza
//!      buildings bordering each real Plaza (from P61), reading "jut
//!      forward into the open space" literally -- a real projection
//!      OUTWARD from the building's own footprint, toward the plaza, not
//!      a recession into it. Runs right after P107 and strictly before
//!      P197/P127/P221 -- all of which need the FINAL building footprint,
//!      not a stale pre-bump one. See p124_activity_pockets's own module
//!      doc for the geometric reading and its own hard-won note on why it
//!      splices the bump into the ring directly rather than using
//!      subtract_convex + union_pieces (the same reassembly-reliability
//!      problem P95/P133 already hit).
//!   10. P197 Thick Walls (once, site-scale): assigns every real building a
//!      real, nonzero `wall_thickness_m`, capped relative to its own real
//!      footprint. Runs after P107 and P124 -- every downstream stage
//!      clones and mutates the buildings P107/P124 produced, so this
//!      field survives untouched to the end. Deliberately scalar-only,
//!      not carved geometry -- see p197_thick_walls's own module doc.
//!   11. P127 Intimacy Gradient (once, site-scale): partitions every
//!      building's ground floor into a depth-ordered sequence of cells
//!      (public wall/entrance bay -> deepest point). Runs right after P107
//!      -- canonical numbering (107 < 127) needs no reordering here. See
//!      `p127_intimacy_gradient`'s own module doc for the full sourced
//!      sequence Alexander's own text lays out (127 -> 128 -> 129 -> 130 ->
//!      131 -> 132 -> 133...).
//!   12. P130 Entrance Room (once, site-scale): tags the cell P127 built at
//!      depth 0.0 as `kind: "entrance"` -- a label only, no geometry
//!      change, see the module's own doc for why. Kept next to P127
//!      instead of Alexander's own post-P129 position since nothing about
//!      it depends on run order relative to P129.
//!   13. P129 Common Areas at the Heart (once, site-scale): marks which of
//!      P127's cells is nearest the plan's center of gravity.
//!   14. P131 The Flow Through Rooms (once, site-scale): connects P127's
//!      cells -- a closed loop for free on courtyard buildings (the ring
//!      already is one), a chain for solid buildings, closed into a real
//!      loop with one passage cell only when short and wide enough (Pattern
//!      132's own cited ~50ft/15m threshold, folded into this operator).
//!   15. P221 (once, site-scale): place real window/door openings on every
//!      building P107 just produced -- floor count from real height, window
//!      bays from real wall geometry, door on whichever wall faces the
//!      nearest street/open space. No randomness. See
//!      `p221_natural_doors_and_windows`'s own module doc for the pattern
//!      graph this closes (P107 -> P159 -> P221).
//!   16. P133 Staircase as a Stage (once, site-scale): carves a real
//!      stair-core strip out of the common-area cell of every multi-story
//!      building, open to the room it interrupts. Runs AFTER P221, not
//!      right after P131 where Alexander's own numbering would put it --
//!      `Building.floors` (this operator's multi-story filter) isn't set
//!      until P221 derives it from real height; running P133 earlier left
//!      every building's `floors` at `None`, so the filter matched nothing
//!      and the pipeline's own `if let Ok(...)` silently swallowed the
//!      resulting error. See the module's own doc for the full story (also
//!      covers the clip_half_plane-based strip technique, borrowed from
//!      P131's own passage cell, and the union_pieces bug it replaced).
//!
//! This is the single real orchestration function; `tests/corrected_pipeline.rs`
//! is the proof it composes end to end on real data, and `examples/dump_pipeline.rs`
//! is what produces fixtures for the external 3D vibe-check
//! (`tools/vibe-render/`). The web UI's "Run full pipeline" button re-implements
//! the same sequence (including the area-proportional square allocation)
//! client-side in JS, since it needs to update the map after each stage
//! rather than only at the end.

use crate::p107_wings_of_light::{P107Params, P107WingsOfLight};
use crate::p108_connected_buildings::{P108ConnectedBuildings, P108Params};
use crate::p127_intimacy_gradient::{P127IntimacyGradient, P127Params};
use crate::p129_common_areas_at_the_heart::{P129CommonAreasAtTheHeart, P129Params};
use crate::p130_entrance_room::{P130EntranceRoom, P130Params};
use crate::p131_the_flow_through_rooms::{P131Params, P131TheFlowThroughRooms};
use crate::p124_activity_pockets::{P124ActivityPockets, P124Params};
use crate::p133_staircase_as_a_stage::{P133Params, P133StaircaseAsAStage};
use crate::p197_thick_walls::{P197Params, P197ThickWalls};
use crate::p221_natural_doors_and_windows::{P221NaturalDoorsAndWindows, P221Params};
use crate::p29_density_rings::{P29DensityRings, P29Params};
use crate::p37_house_cluster::{P37HouseCluster, P37Params};
use crate::p61_small_public_squares::{place_new_squares_n, P61Params, P61SmallPublicSquares};
use crate::p95_building_complex::{P95BuildingComplex, P95Params};
use crate::p96_number_of_stories::{P96NumberOfStories, P96Params};
use crate::path_network::{PathNetwork, PathNetworkParams};
use crate::{apply_subdivision, Parameters, PatternOperator, Subdivision};
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::Scope;

/// Run `f` once per block in `block_ids`, in declared order. `f` receives
/// the neighborhood state as of the start of that block's turn and this
/// block's own derived seed (`base_seed + index + 1` -- the exact
/// convention the P61/P95 per-block loop below already used by hand), and
/// returns an ORDERED sequence of steps for that block (e.g. `[p61_result,
/// p95_result]`), since later steps within one block routinely depend on
/// earlier ones already being applied -- P95 must see the block parcel
/// AFTER P61 has carved a square out of it, not before. `f` is expected to
/// fold its own steps internally (starting from a clone of the state it
/// was given) purely to compute each step correctly against the one
/// before it; `run_per_block` then re-applies that same ordered sequence
/// to its own running state, which is deterministic and produces an
/// identical result without `f` needing write access to the outer loop's
/// state directly. This is that loop's shape, extracted so the next
/// block-scale pattern doesn't have to re-derive it. See
/// PATTERN_LANGUAGE_SIMULATION.md §3.2.
///
/// A step that returns `Err` is skipped, not aborted -- same tolerance the
/// original loop and the web UI's per-block loop apply, since a real
/// rejection (a block too small to be worthwhile) is expected often enough
/// not to be a hard failure. Unlike the original loop, the skip reason is
/// collected rather than silently dropped -- see HARDENING_SPEC.md §1.3,
/// which wanted exactly this so a future check can tell "too small" apart
/// from "geometry op produced garbage" instead of both looking identical
/// after the fact.
pub fn run_per_block<F>(
    nbhd: &Neighborhood,
    block_ids: &[String],
    base_seed: u64,
    mut f: F,
) -> (Neighborhood, Vec<(String, String)>)
where
    F: FnMut(&Neighborhood, &str, u64) -> Vec<Result<Subdivision, String>>,
{
    let mut out = nbhd.clone();
    let mut skipped = Vec::new();
    for (i, block_id) in block_ids.iter().enumerate() {
        let block_seed = base_seed + i as u64 + 1;
        for step in f(&out, block_id, block_seed) {
            match step {
                Ok(sub) => out = apply_subdivision(&out, &sub),
                Err(reason) => skipped.push((block_id.clone(), reason)),
            }
        }
    }
    (out, skipped)
}

/// Split a total square budget across blocks proportional to block area,
/// using largest-remainder rounding so the sum of the returned counts is
/// exactly `total_squares` (not `total_squares` per block). Most blocks in
/// a real site end up with zero -- matching real precedent (Barcelona's
/// superilles place plaza nodes roughly one per several acres, not one per
/// block).
pub fn allocate_squares_by_area(block_areas_m2: &[f64], total_squares: usize) -> Vec<usize> {
    let total_area: f64 = block_areas_m2.iter().sum();
    if total_area <= 0.0 || total_squares == 0 {
        return vec![0; block_areas_m2.len()];
    }
    let raw_shares: Vec<f64> = block_areas_m2.iter()
        .map(|a| total_squares as f64 * a.max(0.0) / total_area)
        .collect();
    let mut counts: Vec<usize> = raw_shares.iter().map(|s| s.floor() as usize).collect();
    let assigned: usize = counts.iter().sum();
    let mut leftover = total_squares.saturating_sub(assigned);

    let mut remainders: Vec<(usize, f64)> = raw_shares.iter().map(|s| s.fract()).enumerate().collect();
    remainders.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    for (i, _) in remainders {
        if leftover == 0 { break; }
        counts[i] += 1;
        leftover -= 1;
    }
    counts
}

/// Run the full corrected pipeline on `parcel_id` within `baseline`, using
/// each operator's default parameters. `seed` drives P37's block seeding and
/// PathNetwork's site-scale steps directly; each block's P61/P95 pass gets
/// a distinct derived seed (`seed + block_index + 1`) so blocks don't all
/// place identical squares.
///
/// A block that fails P61 or P95 (e.g. too small to be worthwhile) is
/// skipped rather than aborting the whole run -- same tolerance the web UI's
/// per-block loop applies.
pub fn run_corrected_pipeline(baseline: &Neighborhood, parcel_id: &str, seed: u64) -> Neighborhood {
    run_corrected_pipeline_with_p37(baseline, parcel_id, seed, &P37Params::defaults())
}

/// Same as `run_corrected_pipeline`, but with an explicit P37 parameter set
/// -- lets callers (currently just `examples/dump_pipeline_seeding.rs`, for
/// comparing P37's `seeding_mode` prototype against production) swap P37's
/// behavior without touching every other stage. `run_corrected_pipeline`
/// itself stays a thin wrapper over this with P37's defaults, so production
/// behavior is unchanged.
pub fn run_corrected_pipeline_with_p37(
    baseline: &Neighborhood,
    parcel_id: &str,
    seed: u64,
    p37_params: &P37Params,
) -> Neighborhood {
    run_corrected_pipeline_with_p37_traced(baseline, parcel_id, seed, p37_params).0
}

/// Same as `run_corrected_pipeline_with_p37`, but also returns the real,
/// literal sequence of operator ids that actually ran -- respecting every
/// `if let Ok(...)` skip -- as it executed, not a hand-copied guess at what
/// the function does. This is what closes `language_graph.rs`'s own
/// documented limitation ("does not, by itself, keep this table in sync
/// with `pipeline.rs`'s actual call sequence"): its test calls this
/// function and validates the returned trace, not a second, independently-
/// maintained literal that could silently drift from this one.
/// `run_corrected_pipeline_with_p37` is a thin wrapper that discards the
/// trace, so every existing caller/test is unaffected.
pub fn run_corrected_pipeline_with_p37_traced(
    baseline: &Neighborhood,
    parcel_id: &str,
    seed: u64,
    p37_params: &P37Params,
) -> (Neighborhood, Vec<&'static str>) {
    let mut trace: Vec<&'static str> = Vec::new();

    let sub37 = P37HouseCluster.apply(baseline, parcel_id, p37_params, seed).unwrap();
    let mut nbhd = apply_subdivision(baseline, &sub37);
    trace.push(P37HouseCluster.name());

    let sub52 = PathNetwork.apply(&nbhd, "*", &PathNetworkParams::defaults(), seed).unwrap();
    nbhd = apply_subdivision(&nbhd, &sub52);
    trace.push(PathNetwork.name());

    if let Ok(sub29) = P29DensityRings.apply(&nbhd, "*", &P29Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub29);
        trace.push(P29DensityRings.name());
    }

    let block_ids: Vec<String> = nbhd.select_ids(&Scope::Block);
    let block_areas: Vec<f64> = block_ids.iter()
        .map(|id| nbhd.parcels.iter().find(|p| &p.id == id).map(|p| p.polygon.area_m2()).unwrap_or(0.0))
        .collect();
    let total_squares = P61Params::defaults().max_squares.round().max(1.0) as usize;
    let square_counts = allocate_squares_by_area(&block_areas, total_squares);

    // Tracked at pipeline-sequence granularity ("did this stage run at all
    // this pipeline pass"), not per-block -- matching what `LANGUAGE`
    // models. A block that's skipped (too small, etc.) doesn't change
    // whether P61/P95 count as having run, as long as at least one block
    // succeeded.
    let p61_ran = std::cell::Cell::new(false);
    let p95_ran = std::cell::Cell::new(false);
    let (folded, _skipped) = run_per_block(&nbhd, &block_ids, seed, |state, block_id, block_seed| {
        let mut steps = Vec::new();
        let mut local = state.clone();
        let n_squares = square_counts[block_ids.iter().position(|b| b == block_id).unwrap()];
        if n_squares > 0 {
            if let Some(block_parcel) = local.parcels.iter().find(|p| &p.id == block_id).cloned() {
                let sub61 = place_new_squares_n(
                    &local, &block_parcel, n_squares, &P61Params::defaults(), block_seed, P61SmallPublicSquares.source(),
                );
                if let Ok(sub) = &sub61 {
                    local = apply_subdivision(&local, sub);
                    p61_ran.set(true);
                }
                steps.push(sub61);
            }
        }
        let sub95 = P95BuildingComplex.apply(&local, block_id, &P95Params::defaults(), block_seed);
        if sub95.is_ok() {
            p95_ran.set(true);
        }
        steps.push(sub95);
        steps
    });
    nbhd = folded;
    if p61_ran.get() {
        trace.push(P61SmallPublicSquares.name());
    }
    if p95_ran.get() {
        trace.push(P95BuildingComplex.name());
    }

    if let Ok(sub108) = P108ConnectedBuildings.apply(&nbhd, "*", &P108Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub108);
        trace.push(P108ConnectedBuildings.name());
    }

    if let Ok(sub96) = P96NumberOfStories.apply(&nbhd, "*", &P96Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub96);
        trace.push(P96NumberOfStories.name());
    }

    if let Ok(sub107) = P107WingsOfLight.apply(&nbhd, "*", &P107Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub107);
        trace.push(P107WingsOfLight.name());
    }

    // P124 Activity Pockets (once, site-scale): bumps a real, small
    // pocket out from up to max_pockets_per_plaza buildings bordering each
    // real Plaza. Runs right after P107 (needs real final building
    // footprints AND real Plazas from P61) and strictly BEFORE P197 (so
    // wall thickness applies to the post-bump footprint), P127 (so
    // interior cells partition the post-bump footprint, not a stale
    // one), and P221 (so window/door ring_index references the final
    // outer ring). Not fatal if no building qualifies -- currently never
    // qualifies on the real eastside-baseline fixture (see
    // p124_activity_pockets's own module doc for the real measured
    // numbers), so this stage is a real no-op there today, not broken.
    if let Ok(sub124) = P124ActivityPockets.apply(&nbhd, "*", &P124Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub124);
        trace.push(P124ActivityPockets.name());
    }

    // P197 Thick Walls (once, site-scale): assigns every real building a
    // real wall_thickness_m, capped relative to its own footprint. Runs
    // after P107 (and after P124, so a bumped building's final footprint
    // is what gets a thickness) -- every downstream stage clones-and-
    // mutates from here, so the field survives to the end of the
    // pipeline untouched. See p197_thick_walls's own module doc.
    if let Ok(sub197) = P197ThickWalls.apply(&nbhd, "*", &P197Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub197);
        trace.push(P197ThickWalls.name());
    }

    if let Ok(sub127) = P127IntimacyGradient.apply(&nbhd, "*", &P127Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub127);
        trace.push(P127IntimacyGradient.name());
    }

    if let Ok(sub130) = P130EntranceRoom.apply(&nbhd, "*", &P130Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub130);
        trace.push(P130EntranceRoom.name());
    }

    if let Ok(sub129) = P129CommonAreasAtTheHeart.apply(&nbhd, "*", &P129Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub129);
        trace.push(P129CommonAreasAtTheHeart.name());
    }

    if let Ok(sub131) = P131TheFlowThroughRooms.apply(&nbhd, "*", &P131Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub131);
        trace.push(P131TheFlowThroughRooms.name());
    }

    if let Ok(sub221) = P221NaturalDoorsAndWindows.apply(&nbhd, "*", &P221Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub221);
        trace.push(P221NaturalDoorsAndWindows.name());
    }

    // AFTER P221, not right after P131 -- Building.floors isn't set until
    // P221 derives it from real height. See p133's own module doc and
    // this file's own doc comment (step 14) for the full story.
    if let Ok(sub133) = P133StaircaseAsAStage.apply(&nbhd, "*", &P133Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub133);
        trace.push(P133StaircaseAsAStage.name());
    }

    (nbhd, trace)
}
