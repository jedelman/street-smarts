//! Diagnostic: run the real corrected pipeline against Eastside Commons,
//! then run the full opinion chorus against the result, and print what
//! the pattern-presence detector opinions actually say about real
//! generated output -- not the hand-built unit fixtures their own tests
//! use, which prove the logic is correct but can't show what it finds on
//! a real run.
//!
//! Kept as a real tool, not a one-off: worth re-running whenever a
//! pattern operator changes, to see how the chorus's read on real output
//! shifts. First real run (three seeds against parcel 00001129) surfaced
//! two things worth knowing about NEW_DETECTORS specifically:
//! - p29_density_rings / p37_house_cluster return NoView on most seeds,
//!   not because they're broken, but because p95_building_complex
//!   consumes (replaces) the BLOCK_n parcel they tag once it builds on
//!   it -- these two opinions can only see something on a seed where a
//!   block happens to survive to the end of the full pipeline. Real
//!   limitation of evaluating the FINAL state, not a detector bug.
//! - p61_small_public_squares scored 0.13-0.18 across all three seeds,
//!   consistently -- worth a real look at whether that's the generator
//!   actually producing oversized squares on this site, or the
//!   detector's sqrt(area) diameter proxy being miscalibrated. Not
//!   determined yet either way.

use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::OpinionOutput;
use street_smarts_opinions::evaluate_all;
use street_smarts_patterns::pipeline::run_corrected_pipeline;

const NEW_DETECTORS: &[&str] = &[
    "p29_density_rings",
    "p37_house_cluster",
    "p61_small_public_squares",
    "p108_connected_buildings",
    "p130_entrance_room",
    "p133_staircase_as_a_stage",
    "p221_natural_doors_and_windows",
    "p115_courtyards_which_live",
    "p112_entrance_transition",
];

fn main() {
    let raw = std::fs::read_to_string("data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");

    for seed in [1u64, 42, 100] {
        println!("\n=== seed {seed} ===");
        let result = run_corrected_pipeline(&baseline, "00001129", seed);
        println!(
            "parcels={} buildings={} open_space={}",
            result.parcels.len(),
            result.buildings.len(),
            result.open_space.len()
        );

        let evaluated = evaluate_all(&result);
        for ev in &evaluated {
            if !NEW_DETECTORS.contains(&ev.opinion.name.as_str()) {
                continue;
            }
            match &ev.output {
                OpinionOutput::Value { value, method_summary, .. } => {
                    println!("  {:32} value={:.3}  {}", ev.opinion.name, value, method_summary);
                }
                OpinionOutput::NoView { reason, .. } => {
                    println!("  {:32} NO_VIEW  {}", ev.opinion.name, reason);
                }
            }
        }
    }
}
