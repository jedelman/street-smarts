//! The corrected pipeline, end to end: Alexander's own pattern numbering
//! (37 < 52 < 61 < 95 < 107) run in that order, instead of the old
//! pipeline's P95 -> BlockGrouping -> PathNetwork -> P107 -> P61.
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
//! This is the single real orchestration function; `tests/corrected_pipeline.rs`
//! is the proof it composes end to end on real data, and `examples/dump_pipeline.rs`
//! is what produces fixtures for the external 3D vibe-check
//! (`tools/vibe-render/`). The web UI's "Run full pipeline" button re-implements
//! the same sequence client-side in JS, since it needs to update the map after
//! each stage rather than only at the end.

use crate::p107_wings_of_light::{P107Params, P107WingsOfLight};
use crate::p37_house_cluster::{P37HouseCluster, P37Params};
use crate::p61_small_public_squares::{P61Params, P61SmallPublicSquares};
use crate::p95_building_complex::{P95BuildingComplex, P95Params};
use crate::path_network::{PathNetwork, PathNetworkParams};
use crate::{apply_subdivision, Parameters, PatternOperator};
use street_smarts_core::nir::Neighborhood;

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

    let block_ids: Vec<String> = nbhd.parcels.iter()
        .filter(|p| p.spec.as_deref().unwrap_or("").starts_with("BLOCK_"))
        .map(|p| p.id.clone())
        .collect();
    for (i, block_id) in block_ids.iter().enumerate() {
        let block_seed = seed + i as u64 + 1;
        if let Ok(sub61) = P61SmallPublicSquares.apply(&nbhd, block_id, &P61Params::defaults(), block_seed) {
            nbhd = apply_subdivision(&nbhd, &sub61);
        }
        if let Ok(sub95) = P95BuildingComplex.apply(&nbhd, block_id, &P95Params::defaults(), block_seed) {
            nbhd = apply_subdivision(&nbhd, &sub95);
        }
    }

    if let Ok(sub107) = P107WingsOfLight.apply(&nbhd, "*", &P107Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub107);
    }

    nbhd
}
