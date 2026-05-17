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
                street_smarts_core::opinion::OpinionOutput::Value { value, method_summary, sub_scores, details, contributing_features, runtime_ms, .. } => {
                    eprintln!("  [{:?}] {}: {:.3}  ({}ms)", ev.opinion.family, ev.opinion.name, value, runtime_ms);
                    eprintln!("    — {}", method_summary);
                    if !sub_scores.is_empty() {
                        eprintln!("    sub-scores:");
                        for (k, v) in sub_scores {
                            eprintln!("      {} = {:.3}", k, v);
                        }
                    }
                    if !details.is_empty() {
                        eprintln!("    details:");
                        for (k, v) in details {
                            eprintln!("      {} = {}", k, v);
                        }
                    }
                    if !contributing_features.is_empty() {
                        eprintln!("    contributing: {}", contributing_features.join(", "));
                    }
                }
                street_smarts_core::opinion::OpinionOutput::NoView { reason, runtime_ms } => {
                    eprintln!("  [{:?}] {}: (abstained, {}ms) {}", ev.opinion.family, ev.opinion.name, runtime_ms, reason);
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
