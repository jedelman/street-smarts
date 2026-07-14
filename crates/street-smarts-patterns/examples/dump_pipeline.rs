//! Runs the corrected pipeline (see `crate::pipeline::run_corrected_pipeline`)
//! against a real fixture parcel and writes the resulting neighborhood as
//! JSON. This is the data-generation half of the 3D "vibe test" -- the
//! render half lives in `tools/vibe-render/render.py`, orchestrated
//! together by `scripts/vibe-render.sh`.
//!
//! Usage:
//!   cargo run -p street-smarts-patterns --release --example dump_pipeline -- \
//!       <fixture.json> <parcel_id> <seed> <out.json>
//!
//! `scripts/vibe-render.sh` calls this once per scenario (the clean
//! baseline block and the fragmented MALL_CORE "barrio breakdown"); run it
//! directly for a one-off dump of any other parcel.

use std::path::Path;
use street_smarts_core::nir::Neighborhood;
use street_smarts_patterns::pipeline::run_corrected_pipeline;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        eprintln!("usage: dump_pipeline <fixture.json> <parcel_id> <seed> <out.json>");
        std::process::exit(1);
    }
    let fixture_path = &args[1];
    let parcel_id = &args[2];
    let seed: u64 = args[3].parse().expect("seed must be a u64");
    let out_path = &args[4];

    let raw = std::fs::read_to_string(fixture_path)
        .unwrap_or_else(|e| panic!("couldn't read {fixture_path}: {e}"));
    let baseline: Neighborhood = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("couldn't parse {fixture_path}: {e}"));

    let result = run_corrected_pipeline(&baseline, parcel_id, seed);

    if let Some(parent) = Path::new(out_path).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(out_path, serde_json::to_string(&result).unwrap())
        .unwrap_or_else(|e| panic!("couldn't write {out_path}: {e}"));

    println!(
        "{out_path}: {} parcels, {} buildings, {} streets, {} open_space",
        result.parcels.len(), result.buildings.len(), result.streets.len(), result.open_space.len()
    );
}
