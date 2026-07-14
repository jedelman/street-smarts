//! Not a real test -- a throwaway dump utility, gated behind an env var so
//! it doesn't run in normal `cargo test`. Runs the corrected pipeline
//! (same sequence as corrected_pipeline.rs) against both the clean baseline
//! parcel and the fragmented MALL_CORE parcel, writing the final
//! neighborhoods to /tmp for an external 3D vibe-check.

use street_smarts_core::nir::Neighborhood;
use street_smarts_patterns::p107_wings_of_light::{P107Params, P107WingsOfLight};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::p61_small_public_squares::{P61Params, P61SmallPublicSquares};
use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};
use street_smarts_patterns::path_network::{PathNetwork, PathNetworkParams};
use street_smarts_patterns::{apply_subdivision, Parameters, PatternOperator};

fn run_corrected_pipeline(baseline: &Neighborhood, parcel_id: &str, seed: u64) -> Neighborhood {
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

#[test]
fn dump_both_scenarios() {
    if std::env::var("DUMP_FOR_VIZ").is_err() {
        eprintln!("skipping dump (set DUMP_FOR_VIZ=1 to run)");
        return;
    }

    let baseline_raw = std::fs::read_to_string("../../data/eastside-baseline.json").unwrap();
    let baseline: Neighborhood = serde_json::from_str(&baseline_raw).unwrap();
    let clean = run_corrected_pipeline(&baseline, "00001129", 42);
    std::fs::write("/tmp/viz_clean_baseline.json", serde_json::to_string(&clean).unwrap()).unwrap();
    eprintln!("clean baseline: {} parcels, {} buildings, {} streets, {} open_space", clean.parcels.len(), clean.buildings.len(), clean.streets.len(), clean.open_space.len());

    let proposal_raw = std::fs::read_to_string("../../data/eastside-proposal.json").unwrap();
    let proposal: Neighborhood = serde_json::from_str(&proposal_raw).unwrap();
    let barrio = run_corrected_pipeline(&proposal, "13279568", 42);
    std::fs::write("/tmp/viz_barrio_mallcore.json", serde_json::to_string(&barrio).unwrap()).unwrap();
    eprintln!("barrio mall_core: {} parcels, {} buildings, {} streets, {} open_space", barrio.parcels.len(), barrio.buildings.len(), barrio.streets.len(), barrio.open_space.len());
}
