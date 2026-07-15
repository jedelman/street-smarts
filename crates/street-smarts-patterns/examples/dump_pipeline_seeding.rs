//! Runs the corrected pipeline twice on the same fixture/parcel/seed --
//! once with P37's default Stratified seeding, once with the FieldGuided
//! prototype (`seeding_mode=1.0`) -- and writes both as JSON, so the two
//! can be rendered side by side with `tools/vibe-render/render.py`. See
//! `examples/dump_pipeline.rs` for the single-run version this is based on.
//!
//! Usage:
//!   cargo run -p street-smarts-patterns --release --example dump_pipeline_seeding -- \
//!       <fixture.json> <parcel_id> <seed> <out_prefix>
//!
//! Writes `<out_prefix>_stratified.json` and `<out_prefix>_fieldguided.json`.

use std::path::Path;
use street_smarts_core::nir::Neighborhood;
use street_smarts_patterns::p37_house_cluster::P37Params;
use street_smarts_patterns::pipeline::run_corrected_pipeline_with_p37;
use street_smarts_patterns::Parameters;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        eprintln!("usage: dump_pipeline_seeding <fixture.json> <parcel_id> <seed> <out_prefix>");
        std::process::exit(1);
    }
    let fixture_path = &args[1];
    let parcel_id = &args[2];
    let seed: u64 = args[3].parse().expect("seed must be a u64");
    let out_prefix = &args[4];

    let raw = std::fs::read_to_string(fixture_path)
        .unwrap_or_else(|e| panic!("couldn't read {fixture_path}: {e}"));
    let baseline: Neighborhood = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("couldn't parse {fixture_path}: {e}"));

    if let Some(parent) = Path::new(out_prefix).parent() {
        std::fs::create_dir_all(parent).ok();
    }

    for (label, params) in [
        ("stratified", P37Params::defaults()),
        ("fieldguided", P37Params { seeding_mode: 1.0, ..P37Params::defaults() }),
    ] {
        let result = run_corrected_pipeline_with_p37(&baseline, parcel_id, seed, &params);
        let out_path = format!("{out_prefix}_{label}.json");
        std::fs::write(&out_path, serde_json::to_string(&result).unwrap())
            .unwrap_or_else(|e| panic!("couldn't write {out_path}: {e}"));
        println!(
            "{out_path}: {} parcels, {} buildings, {} streets, {} open_space",
            result.parcels.len(), result.buildings.len(), result.streets.len(), result.open_space.len()
        );
    }
}
