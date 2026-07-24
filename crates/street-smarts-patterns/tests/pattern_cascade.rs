//! Operationalizes Alexander's own cross-reference DAG through *A Pattern
//! Language* (the graph patternlanguage.cc's own site renders) as a real
//! regression check: runs the full corrected pipeline ONCE against the
//! real `eastside-baseline.json` fixture, then verifies every
//! `cascade_contracts::CASCADE_CONTRACTS` entry actually holds -- that a
//! specific generator's real output really does move a specific
//! downstream opinion's real score (or, where the opinion's own number
//! doesn't move on this particular fixture, that the real structural fact
//! it depends on genuinely exists).
//!
//! Before this test existed, each of these was verified once by hand in a
//! throwaway `examples/check_*.rs` script, the real number got read into a
//! commit message, and the script was deleted -- nothing would have caught
//! a later regression. See `cascade_contracts.rs`'s own module doc for the
//! full rationale.

use street_smarts_core::nir::Neighborhood;
use street_smarts_patterns::cascade_contracts::{CascadeCheck, CASCADE_CONTRACTS};
use street_smarts_patterns::p37_house_cluster::P37Params;
use street_smarts_patterns::pipeline::run_corrected_pipeline_with_p37;
use street_smarts_patterns::Parameters;

#[test]
fn every_cascade_contract_holds_on_the_real_fixture() {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");
    let final_nbhd = run_corrected_pipeline_with_p37(
        &baseline,
        "MILITARY_CIRCLE_ASSEMBLED",
        42,
        &P37Params::defaults(),
    );

    let evaluated = street_smarts_opinions::evaluate_all(&final_nbhd);
    let mut failures: Vec<String> = Vec::new();

    for contract in CASCADE_CONTRACTS {
        match &contract.check {
            CascadeCheck::MinValue(floor) => {
                let found = evaluated.iter().find(|e| e.opinion.name == contract.opinion);
                match found {
                    Some(e) => match &e.output {
                        street_smarts_core::opinion::OpinionOutput::Value { value, .. } => {
                            if *value < *floor {
                                failures.push(format!(
                                    "{} (P{}) <- {} (P{}): expected Value >= {floor:.3}, got {value:.3}. {}",
                                    contract.opinion, contract.opinion_pattern,
                                    contract.generator, contract.generator_pattern,
                                    contract.why
                                ));
                            }
                        }
                        street_smarts_core::opinion::OpinionOutput::NoView { reason, .. } => {
                            failures.push(format!(
                                "{} (P{}) <- {} (P{}): expected Value >= {floor:.3}, got NoView ({reason}). {}",
                                contract.opinion, contract.opinion_pattern,
                                contract.generator, contract.generator_pattern,
                                contract.why
                            ));
                        }
                    },
                    None => failures.push(format!(
                        "{} (P{}): opinion not found in the real registry -- renamed or removed?",
                        contract.opinion, contract.opinion_pattern
                    )),
                }
            }
            CascadeCheck::StructuralFact(pred) => {
                if !pred(&final_nbhd) {
                    failures.push(format!(
                        "{} (P{}) <- {} (P{}): real structural fact no longer holds on the real fixture. {}",
                        contract.opinion, contract.opinion_pattern,
                        contract.generator, contract.generator_pattern,
                        contract.why
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "cascade contract violation(s) -- a generator's real output no longer reaches its \
         downstream detector as expected:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_cascade_contract_generator_is_a_real_language_graph_node() {
    // Guards against a contract citing a generator id that doesn't (or no
    // longer) exists in the real pipeline sequence -- language_graph's own
    // LANGUAGE table is the single source of truth for real generator ids.
    for contract in CASCADE_CONTRACTS {
        assert!(
            street_smarts_patterns::language_graph::LANGUAGE.iter().any(|n| n.id == contract.generator),
            "cascade contract for {} cites generator '{}', which isn't a real language_graph::LANGUAGE node",
            contract.opinion, contract.generator
        );
    }
}
