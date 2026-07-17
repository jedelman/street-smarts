//! Shared property-fuzzing harness — PATTERN_LANGUAGE_SIMULATION.md §4.4.
//!
//! Generalizes the existing pattern of "hand-write one assertion against
//! one hardcoded seed" (see `tests/p37_house_cluster.rs`'s
//! `carves_a_large_parcel_into_several_blocks`, seed 7 only) into running
//! the same invariant across many seeds. A pattern only earns real status
//! by holding across many real cases, not one -- a single hardcoded seed
//! structurally can't test that (see PATTERN_LANGUAGE_SIMULATION.md §4.4's
//! own framing). This module covers the SEED axis directly; the SHAPE
//! axis (varying the input fixture, not just the RNG) is
//! `synthetic_fixtures` below, per `HARDENING_SPEC.md` §5.

pub mod synthetic_fixtures;

/// Run `f` once per seed in `seeds`, collecting every seed that fails
/// (returns `Err`) rather than panicking on the first one -- a caller
/// gets the full failure picture (which seeds, how many) in one run
/// instead of fixing one at a time.
pub fn assert_invariant_across_seeds<F>(seeds: &[u64], mut f: F)
where
    F: FnMut(u64) -> Result<(), String>,
{
    let mut failures: Vec<(u64, String)> = Vec::new();
    for &seed in seeds {
        if let Err(reason) = f(seed) {
            failures.push((seed, reason));
        }
    }
    assert!(
        failures.is_empty(),
        "invariant failed for {}/{} seed(s):\n{}",
        failures.len(),
        seeds.len(),
        failures.iter().map(|(s, r)| format!("  seed {s}: {r}")).collect::<Vec<_>>().join("\n")
    );
}

/// A spread of seeds wide enough to catch seed-dependent RNG edge cases
/// without making every test slow -- small, prime-ish, deliberately not
/// just 0..N (avoids accidentally relying on sequential-seed structure).
pub const DEFAULT_SEEDS: &[u64] = &[1, 2, 3, 7, 13, 29, 41, 97, 101, 233];
