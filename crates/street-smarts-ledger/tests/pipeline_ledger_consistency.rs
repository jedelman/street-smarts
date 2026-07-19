//! `run_corrected_pipeline_via_ledger` and `pipeline.rs`'s own
//! `run_corrected_pipeline` are necessarily two separate implementations
//! of the same 14-stage sequence -- `street-smarts-patterns` (the real
//! production pipeline `street-smarts-web`'s WASM build ships) can't
//! depend on `street-smarts-ledger` without inverting the crate graph, so
//! the ledger-based version this crate's `examples/dump_pipeline.rs` and
//! `examples/dump_lineage_animation.rs` both now share can't literally
//! BE `pipeline.rs`'s function. See `corrected_pipeline.rs`'s own module
//! doc for the full rationale.
//!
//! This is the tripwire for that real, accepted duplication: both
//! implementations, run against the same real fixture/parcel/seed, must
//! produce byte-for-byte identical final `Neighborhood`s. If a future
//! change to either sequence (a reordered stage, a changed default
//! param, a new stage added to one but not the other) breaks that, this
//! test fails immediately instead of the two silently drifting apart and
//! only being noticed when the public gallery's lineage animation stops
//! matching its own final-state render.

use street_smarts_core::nir::Neighborhood;
use street_smarts_ledger::{run_corrected_pipeline_via_ledger, HistoryStore, InMemoryHistoryStore};
use street_smarts_patterns::pipeline::run_corrected_pipeline;

fn load_fixture(path: &str) -> Neighborhood {
    let raw = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("couldn't read {path}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("couldn't parse {path}: {e}"))
}

fn assert_pipelines_match(fixture_path: &str, parcel_id: &str, seed: u64) {
    let baseline = load_fixture(fixture_path);

    let direct = run_corrected_pipeline(&baseline, parcel_id, seed);

    let mut store = InMemoryHistoryStore::new();
    let root_id = store.insert_root(&baseline);
    let (final_id, commits) = run_corrected_pipeline_via_ledger(&mut store, root_id, parcel_id, seed);
    let via_ledger = store.materialize(&final_id).expect("final commit must materialize");

    assert!(!commits.is_empty(), "the ledger-based run should have recorded at least one real commit");
    assert_eq!(
        direct, via_ledger,
        "run_corrected_pipeline (direct) and run_corrected_pipeline_via_ledger (commit-by-commit) \
         produced DIFFERENT final neighborhoods for the same (fixture, parcel, seed) -- the two \
         implementations of the 14-stage sequence have drifted apart. See this test's own module \
         doc and corrected_pipeline.rs's."
    );
}

#[test]
fn clean_baseline_matches_between_direct_and_ledger_execution() {
    assert_pipelines_match("../../data/eastside-baseline.json", "MILITARY_CIRCLE_ASSEMBLED", 42);
}

#[test]
fn barrio_mallcore_matches_between_direct_and_ledger_execution() {
    assert_pipelines_match("../../data/eastside-proposal.json", "13279568", 42);
}
