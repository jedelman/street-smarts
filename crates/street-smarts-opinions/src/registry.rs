//! Registry of v0.1 opinions and a helper to evaluate them all.

use crate::activist::OwnershipPattern;
use crate::geometric::{LevelsOfScale, StrongCenters};
use serde::{Deserialize, Serialize};
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, OpinionRef};

/// One evaluated opinion, ready for the conflict engine and the renderer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatedOpinion {
    pub opinion: OpinionRef,
    pub output: OpinionOutput,
}

/// Build the v0.1 opinion roster as boxed trait objects.
pub fn all_opinions_v01() -> Vec<Box<dyn Opinion>> {
    vec![
        Box::new(LevelsOfScale),
        Box::new(StrongCenters),
        Box::new(OwnershipPattern),
    ]
}

/// Run all v0.1 opinions against a neighborhood and collect results.
pub fn evaluate_all(n: &Neighborhood) -> Vec<EvaluatedOpinion> {
    all_opinions_v01()
        .into_iter()
        .map(|op| {
            let opinion_ref = Opinion::as_ref(op.as_ref());
            let output = op.evaluate(n);
            EvaluatedOpinion {
                opinion: opinion_ref,
                output,
            }
        })
        .collect()
}

/// Group evaluated opinions by family — used by the conflict engine to
/// keep the geometric chorus separate from the activist guards.
pub fn group_by_family(
    evaluated: &[EvaluatedOpinion],
) -> std::collections::HashMap<OpinionFamily, Vec<&EvaluatedOpinion>> {
    let mut groups: std::collections::HashMap<OpinionFamily, Vec<&EvaluatedOpinion>> =
        std::collections::HashMap::new();
    for ev in evaluated {
        groups.entry(ev.opinion.family).or_default().push(ev);
    }
    groups
}
