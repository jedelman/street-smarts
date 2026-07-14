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
//!      inherits its source block's P29 density tier/target.
//!   6. P96 Number of Stories (once, site-scale): turns each pad's
//!      inherited tier target into a real per-pad story count, capping
//!      ordinary buildings at 4 stories (P21 Four-Story Limit) with a very
//!      few, widely-spaced exceptions where a tier's target calls for more.
//!   7. P107 (once, site-scale): shape every P95 pad for daylight depth,
//!      reading P96's `target_stories` assignment for real height -- unless
//!      P96 didn't run, the flat `assumed_height_m` fallback applies
//!      exactly as before P96 existed. Already filters by
//!      `use_category == "p95_building_pad"` across the whole neighborhood
//!      regardless of which block a pad came from.
//!
//! This is the single real orchestration function; `tests/corrected_pipeline.rs`
//! is the proof it composes end to end on real data, and `examples/dump_pipeline.rs`
//! is what produces fixtures for the external 3D vibe-check
//! (`tools/vibe-render/`). The web UI's "Run full pipeline" button re-implements
//! the same sequence (including the area-proportional square allocation)
//! client-side in JS, since it needs to update the map after each stage
//! rather than only at the end.

use crate::p107_wings_of_light::{P107Params, P107WingsOfLight};
use crate::p29_density_rings::{P29DensityRings, P29Params};
use crate::p37_house_cluster::{P37HouseCluster, P37Params};
use crate::p61_small_public_squares::{place_new_squares_n, P61Params, P61SmallPublicSquares};
use crate::p95_building_complex::{P95BuildingComplex, P95Params};
use crate::p96_number_of_stories::{P96NumberOfStories, P96Params};
use crate::path_network::{PathNetwork, PathNetworkParams};
use crate::{apply_subdivision, Parameters, PatternOperator};
use street_smarts_core::nir::Neighborhood;

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
    let sub37 = P37HouseCluster.apply(baseline, parcel_id, &P37Params::defaults(), seed).unwrap();
    let mut nbhd = apply_subdivision(baseline, &sub37);

    let sub52 = PathNetwork.apply(&nbhd, "*", &PathNetworkParams::defaults(), seed).unwrap();
    nbhd = apply_subdivision(&nbhd, &sub52);

    if let Ok(sub29) = P29DensityRings.apply(&nbhd, "*", &P29Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub29);
    }

    let block_ids: Vec<String> = nbhd.parcels.iter()
        .filter(|p| p.spec.as_deref().unwrap_or("").starts_with("BLOCK_"))
        .map(|p| p.id.clone())
        .collect();
    let block_areas: Vec<f64> = block_ids.iter()
        .map(|id| nbhd.parcels.iter().find(|p| &p.id == id).map(|p| p.polygon.area_m2()).unwrap_or(0.0))
        .collect();
    let total_squares = P61Params::defaults().max_squares.round().max(1.0) as usize;
    let square_counts = allocate_squares_by_area(&block_areas, total_squares);

    for (i, block_id) in block_ids.iter().enumerate() {
        let block_seed = seed + i as u64 + 1;
        let n_squares = square_counts[i];
        if n_squares > 0 {
            if let Some(block_parcel) = nbhd.parcels.iter().find(|p| &p.id == block_id).cloned() {
                if let Ok(sub61) = place_new_squares_n(
                    &nbhd, &block_parcel, n_squares, &P61Params::defaults(), block_seed, P61SmallPublicSquares.source(),
                ) {
                    nbhd = apply_subdivision(&nbhd, &sub61);
                }
            }
        }
        if let Ok(sub95) = P95BuildingComplex.apply(&nbhd, block_id, &P95Params::defaults(), block_seed) {
            nbhd = apply_subdivision(&nbhd, &sub95);
        }
    }

    if let Ok(sub96) = P96NumberOfStories.apply(&nbhd, "*", &P96Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub96);
    }

    if let Ok(sub107) = P107WingsOfLight.apply(&nbhd, "*", &P107Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub107);
    }

    nbhd
}
