//! P51 Green Streets — local, no-through-traffic streets should be grass
//! with paving stones set into it for wheels, not solid asphalt.
//!
//! From Alexander, *A Pattern Language*, Pattern 51 (p. 266), via
//! patternlanguage.cc/Patterns/Green-Streets-(51):
//! > **Problem:** There is too much hot hard asphalt in the world. A local
//! > road, which only gives access to buildings, needs a few stones for
//! > the wheels of the cars; nothing more. Most of it can still be green.
//! > **Solution:** On local roads, closed to through traffic, plant grass
//! > all over the road and set occasional paving stones into the grass to
//! > form a surface for the wheels of those cars that need access to the
//! > street. Make no distinction between street and sidewalk.
//!
//! # A real, honest gap -- not a proxy
//!
//! `Street` models `id`, `centerline`, `classification`, and `row_width_m`
//! only -- no surface-material field exists anywhere in this pipeline's
//! schema, for any street classification. This opinion exists, with a
//! real citation, specifically to be honest about that: it always returns
//! `NoView` with the real reason, rather than inventing a proxy that
//! isn't real data.

use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P51GreenStreets;

impl Opinion for P51GreenStreets {
    fn name(&self) -> &'static str {
        "p51_green_streets"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p51".into(),
            display: "Alexander et al., A Pattern Language, Pattern 51 (Green Streets)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Green-Streets-(51)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, _n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();
        OpinionOutput::NoView {
            reason: "No street-surface-material field exists anywhere in this pipeline's schema -- \
                     Street models only id, centerline, classification, and row_width_m, with \
                     nothing to distinguish asphalt from grass-and-pavers. See this opinion's own \
                     module doc."
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
                layer_provenance: Default::default(), label: "P51 unit fixture".into(),
            },
        };
        match P51GreenStreets.evaluate(&n) {
            OpinionOutput::NoView { reason, .. } => assert!(reason.contains("surface-material")),
            other => panic!("expected NoView, got {other:?}"),
        }
    }
}
