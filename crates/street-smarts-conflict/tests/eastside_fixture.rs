//! Integration test: load Eastside Commons fixtures and run the v0.1 opinions.

use std::fs;

#[test]
fn evaluate_eastside_baseline_and_proposal() {
    let baseline_path = "../../data/eastside-baseline.json";
    let proposal_path = "../../data/eastside-proposal.json";

    let baseline_json = fs::read_to_string(baseline_path).expect("baseline fixture missing");
    let proposal_json = fs::read_to_string(proposal_path).expect("proposal fixture missing");

    let baseline: street_smarts_core::nir::Neighborhood =
        serde_json::from_str(&baseline_json).expect("baseline parse");
    let proposal: street_smarts_core::nir::Neighborhood =
        serde_json::from_str(&proposal_json).expect("proposal parse");

    println!("\n=== BASELINE ({} parcels) ===", baseline.parcels.len());
    for ev in street_smarts_opinions::evaluate_all(&baseline) {
        println!(
            "{} [{:?}] → {}",
            ev.opinion.name,
            ev.opinion.family,
            describe(&ev.output)
        );
    }

    println!("\n=== PROPOSAL ({} parcels) ===", proposal.parcels.len());
    for ev in street_smarts_opinions::evaluate_all(&proposal) {
        println!(
            "{} [{:?}] → {}",
            ev.opinion.name,
            ev.opinion.family,
            describe(&ev.output)
        );
    }

    // Build conflict reports
    let baseline_eval = street_smarts_opinions::evaluate_all(&baseline);
    let proposal_eval = street_smarts_opinions::evaluate_all(&proposal);
    let baseline_report = street_smarts_conflict::build_report(baseline_eval);
    let proposal_report = street_smarts_conflict::build_report(proposal_eval);

    println!("\n=== BASELINE CONFLICT REPORT ===");
    println!("Geometric: {}", baseline_report.geometric_summary.headline);
    println!("Activist:  {}", baseline_report.activist_summary.headline);
    println!("Questions:");
    for q in &baseline_report.questions_for_humans {
        println!("  - {}", q.question);
    }

    println!("\n=== PROPOSAL CONFLICT REPORT ===");
    println!("Geometric: {}", proposal_report.geometric_summary.headline);
    println!("Activist:  {}", proposal_report.activist_summary.headline);
    println!("Questions:");
    for q in &proposal_report.questions_for_humans {
        println!("  - {}", q.question);
    }
    println!("Abstentions:");
    for a in &proposal_report.abstentions {
        println!("  - {}: {}", a.opinion_name, a.reason);
    }
}

fn describe(out: &street_smarts_core::opinion::OpinionOutput) -> String {
    match out {
        street_smarts_core::opinion::OpinionOutput::Value { value, method_summary, .. } => {
            format!("VALUE {:.3} — {}", value, method_summary)
        }
        street_smarts_core::opinion::OpinionOutput::NoView { reason, .. } => {
            format!("NO VIEW — {}", reason)
        }
    }
}
