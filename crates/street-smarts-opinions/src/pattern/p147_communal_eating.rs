//! P147 Communal Eating — a group needs a regular, real occasion of
//! eating together to hold together.
//!
//! From Alexander, *A Pattern Language*, Pattern 147 (p. 696), via
//! patternlanguage.cc/Patterns/Communal-Eating-(147):
//! > **Problem:** Without communal eating, no human group can hold
//! > together.
//! > **Solution:** Establish dedicated spaces and regular occasions for
//! > shared meals within institutions and workplaces... make communal
//! > lunch a daily practice around a proper table... rotate cooking
//! > responsibilities among group members.
//!
//! # A real, honest gap -- not a proxy
//!
//! Every real claim in this pattern is an institutional PRACTICE and
//! SCHEDULE -- "daily," "regular occasion," "rotating responsibilities"
//! -- not a geometric or spatial claim this pipeline could ever check
//! from a `Neighborhood`'s own data. There's no schedule, program, or
//! social-practice data anywhere in this schema, and there's no real
//! geometric stand-in to reach for either (unlike `p139_farmhouse_kitchen.rs`,
//! which at least has a real common-cell AREA to check as a loose,
//! honestly-caveated proxy for "a proper table fits"). This opinion
//! exists, with a real citation, specifically to be honest about that:
//! it always returns `NoView` with the real reason, rather than
//! inventing a proxy that isn't real data.

use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P147CommunalEating;

impl Opinion for P147CommunalEating {
    fn name(&self) -> &'static str {
        "p147_communal_eating"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p147".into(),
            display: "Alexander et al., A Pattern Language, Pattern 147 (Communal Eating)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Communal-Eating-(147)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, _n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();
        OpinionOutput::NoView {
            reason: "Every real claim in this pattern is an institutional practice and schedule \
                     (a daily communal lunch, rotating cooking responsibilities), not a geometric \
                     claim -- there's no schedule/program/social-practice data anywhere in this \
                     pipeline's schema, and no honest geometric stand-in to check instead. See this \
                     opinion's own module doc."
                .into(),
            runtime_ms: timer.elapsed_ms(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::nir::NeighborhoodMeta;

    #[test]
    fn always_returns_no_view_with_the_real_reason() {
        let n = Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings: vec![], streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P147 unit fixture".into(),
            },
        };
        match P147CommunalEating.evaluate(&n) {
            OpinionOutput::NoView { reason, .. } => {
                assert!(reason.contains("practice") || reason.contains("schedule"), "got: {reason}");
            }
            other => panic!("expected NoView, got {other:?}"),
        }
    }
}
