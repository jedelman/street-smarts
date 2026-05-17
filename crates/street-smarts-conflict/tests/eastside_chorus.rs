//! Sanity check: load EC baseline + proposal fixtures, run all opinions,
//! report what the chorus says. Cheap way to validate before WASM.

use std::fs;
use street_smarts_conflict::build_report;
use street_smarts_core::nir::Neighborhood;
use street_smarts_opinions::evaluate_all;

#[test]
fn eastside_commons_chorus() {
    for (name, path) in [
        ("BASELINE (current parcel fabric)", "../../data/eastside-baseline.json"),
        ("PROPOSAL (EC_FieldSolver output)", "../../data/eastside-proposal.json"),
    ] {
        let raw = fs::read_to_string(path).expect("fixture present");
        let n: Neighborhood = serde_json::from_str(&raw).expect("parseable");
        let evaluated = evaluate_all(&n);
        let report = build_report(evaluated);

        eprintln!("\n========== {} ==========", name);
        eprintln!("Parcels: {}", n.parcels.len());
        eprintln!("\n-- Geometric chorus --");
        eprintln!("{}", report.geometric_summary.headline);
        eprintln!("-- Activist chorus --");
        eprintln!("{}", report.activist_summary.headline);
        eprintln!("\n-- Individual voices --");
        for ev in &report.opinions {
            match &ev.output {
                street_smarts_core::opinion::OpinionOutput::Value { value, method_summary, .. } => {
                    eprintln!("  [{:?}] {}: {:.3}  — {}", ev.opinion.family, ev.opinion.name, value, method_summary);
                }
                street_smarts_core::opinion::OpinionOutput::NoView { reason, .. } => {
                    eprintln!("  [{:?}] {}: (abstained) {}", ev.opinion.family, ev.opinion.name, reason);
                }
            }
        }
        eprintln!("\n-- Questions the conflict engine surfaces --");
        for q in &report.questions_for_humans {
            eprintln!("  ? {}", q.question);
            eprintln!("    why: {}", q.why_it_matters);
        }
    }
}
