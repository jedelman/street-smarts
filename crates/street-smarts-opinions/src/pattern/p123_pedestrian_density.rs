//! P123 Pedestrian Density — a public gathering space should be sized to
//! the real number of people using it, neither empty nor overcrowded.
//!
//! From Alexander, *A Pattern Language*, Pattern 123 (p. 596), via
//! patternlanguage.cc/Patterns/Pedestrian-Density-(123):
//! > **Problem:** Many of our modern public squares, though intended as
//! > lively plazas, are in fact deserted and dead.
//! > **Solution:** For public gathering spaces like squares, courts, and
//! > pedestrian streets, calculate the mean number of people present at
//! > any moment (P), then size the area between 150P and 300P square
//! > feet to maintain vitality.
//!
//! # A real, honest gap -- not a proxy
//!
//! Alexander's own literal test needs `P`: the real mean number of
//! people actually present in a space at a given moment. Nothing in this
//! pipeline simulates foot traffic, occupancy, or any people-count at
//! all -- there's no population, dwelling-unit, or activity-level data
//! anywhere in the schema to derive `P` from, even loosely. A plaza's
//! own AREA alone can't stand in for this without silently assuming a
//! `P` value backwards from the area -- that isn't a real check, it's a
//! tautology dressed as one.
//!
//! Unlike this project's other structurally-detector-only patterns
//! (P32/P46/P89's "commercial" tag, at least a real if unpopulated field
//! that COULD carry real data one day), there's no real field this
//! pattern could ever check against without this pipeline first growing
//! an actual occupancy/foot-traffic model -- a substantially different
//! kind of capability than any generator here produces. This opinion
//! exists, with a real citation, specifically to be honest about that:
//! it always returns `NoView` with the real reason, rather than
//! inventing a proxy that isn't real data.

use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P123PedestrianDensity;

impl Opinion for P123PedestrianDensity {
    fn name(&self) -> &'static str {
        "p123_pedestrian_density"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p123".into(),
            display: "Alexander et al., A Pattern Language, Pattern 123 (Pedestrian Density)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Pedestrian-Density-(123)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, _n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();
        OpinionOutput::NoView {
            reason: "Alexander's own literal test needs P, the real mean number of people present \
                     in a space -- nothing in this pipeline simulates foot traffic, occupancy, or \
                     population at all, and a plaza's own area alone can't honestly stand in for \
                     that without assuming the answer backwards. See this opinion's own module doc."
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
                layer_provenance: Default::default(), label: "P123 unit fixture".into(),
            },
        };
        match P123PedestrianDensity.evaluate(&n) {
            OpinionOutput::NoView { reason, .. } => {
                assert!(reason.contains("foot traffic") || reason.contains("occupancy"), "got: {reason}");
            }
            other => panic!("expected NoView, got {other:?}"),
        }
    }
}
