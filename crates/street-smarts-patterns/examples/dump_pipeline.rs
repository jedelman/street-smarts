//! Runs the corrected pipeline against a real fixture parcel and writes
//! the resulting neighborhood as JSON. This is the data-generation half
//! of the 3D "vibe test" -- the render half lives in
//! `tools/vibe-render/render.py`, orchestrated together by
//! `scripts/vibe-render.sh`.
//!
//! Runs through `street_smarts_ledger::run_corrected_pipeline_via_ledger`
//! -- the SAME function `examples/dump_lineage_animation.rs` (in the
//! `street-smarts-ledger` crate) uses -- rather than calling
//! `crate::pipeline::run_corrected_pipeline` directly, so a single real
//! pipeline run produces both this JSON and, when the animation is also
//! generated for the same scenario, its every intermediate frame. Before
//! this, both examples independently recomputed the identical 14-stage
//! sequence from scratch; see `run_corrected_pipeline_via_ledger`'s own
//! doc comment for the full rationale. `street-smarts-patterns` itself
//! stays free of any dependency on `street-smarts-ledger` in real library
//! terms (this is a `[dev-dependencies]`-only link, used by this example
//! alone) -- the ledger crate depends on patterns, not the reverse, so
//! the real production pipeline (`pipeline.rs`, what `street-smarts-web`'s
//! WASM build actually ships) can't itself route through the ledger
//! without an inverted dependency. `tests/pipeline_ledger_consistency.rs`
//! (in `street-smarts-ledger`) is the tripwire that catches the two
//! implementations drifting apart, since this real constraint means
//! they're necessarily two separate copies of the same sequence, not one.
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
use street_smarts_ledger::{run_corrected_pipeline_via_ledger, HistoryStore, InMemoryHistoryStore};

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

    let mut store = InMemoryHistoryStore::new();
    let root_id = store.insert_root(&baseline);
    let (final_id, _commits) = run_corrected_pipeline_via_ledger(&mut store, root_id, parcel_id, seed);
    let result = store.materialize(&final_id).expect("final commit must materialize");

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
