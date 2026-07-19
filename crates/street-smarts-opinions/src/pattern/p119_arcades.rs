//! P119 Arcades — covered walkways at a building's edge should connect
//! buildings to one another, part inside, part outside.
//!
//! From Alexander, *A Pattern Language*, Pattern 119 (p. 580), via
//! patternlanguage.cc/Patterns/Arcades-(119):
//! > **Problem:** Arcades -- covered walkways at the edge of buildings,
//! > which are partly inside, partly outside -- play a vital role in the
//! > way that people interact with buildings.
//! > **Solution:** Wherever paths run along the edge of buildings, build
//! > arcades, and use the arcades, above all, to connect up the
//! > buildings to one another.
//!
//! # A real, honest gap -- not a proxy
//!
//! An arcade is a real, distinct covered-canopy structure attached to a
//! building's edge -- nothing like it exists anywhere in this pipeline's
//! schema (`Building` has walls and openings, no canopy/awning/covered-
//! walkway concept at all). This opinion exists, with a real citation,
//! specifically to be honest about that: it always returns `NoView` with
//! the real reason, rather than inventing a proxy that isn't real data.

use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P119Arcades;

impl Opinion for P119Arcades {
    fn name(&self) -> &'static str {
        "p119_arcades"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p119".into(),
            display: "Alexander et al., A Pattern Language, Pattern 119 (Arcades)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Arcades-(119)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, _n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();
        OpinionOutput::NoView {
            reason: "No covered-canopy/awning/arcade concept exists anywhere in this pipeline's \
                     schema -- Building only models walls and openings, nothing attached to a wall's \
                     edge. See this opinion's own module doc."
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
                layer_provenance: Default::default(), label: "P119 unit fixture".into(),
            },
        };
        match P119Arcades.evaluate(&n) {
            OpinionOutput::NoView { reason, .. } => assert!(reason.contains("arcade") || reason.contains("canopy"), "got: {reason}"),
            other => panic!("expected NoView, got {other:?}"),
        }
    }
}
