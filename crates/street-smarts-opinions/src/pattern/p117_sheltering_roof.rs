//! P117 Sheltering Roof — the roof should be visible, sloped, and felt
//! as real shelter, with eaves brought down low at entrances.
//!
//! From Alexander, *A Pattern Language*, Pattern 117 (p. 569), via
//! patternlanguage.cc/Patterns/Sheltering-Roof-(117):
//! > **Problem:** The roof plays a primal role in our lives. If the roof
//! > is hidden, if its presence cannot be felt around the building, or
//! > if it cannot be used, then people will lack a fundamental sense of
//! > shelter.
//! > **Solution:** Slope the roof or make a vault of it, make its entire
//! > surface visible, and bring the eaves of the roof down low.
//!
//! # A real, honest gap -- not a proxy
//!
//! Same real gap as `p116_cascade_of_roofs.rs`: this pipeline models no
//! roof geometry at all -- every building is a flat-topped extrusion,
//! with nothing resembling a slope, vault, or eave anywhere in the
//! schema. This opinion exists, with a real citation, specifically to be
//! honest about that: it always returns `NoView` with the real reason,
//! rather than inventing a proxy that isn't real data.

use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P117ShelteringRoof;

impl Opinion for P117ShelteringRoof {
    fn name(&self) -> &'static str {
        "p117_sheltering_roof"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p117".into(),
            display: "Alexander et al., A Pattern Language, Pattern 117 (Sheltering Roof)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Sheltering-Roof-(117)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, _n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();
        OpinionOutput::NoView {
            reason: "This pipeline models no roof geometry at all -- no slope, vault, or eave field \
                     exists anywhere in the schema to check a sheltering-roof claim against. See this \
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
                layer_provenance: Default::default(), label: "P117 unit fixture".into(),
            },
        };
        match P117ShelteringRoof.evaluate(&n) {
            OpinionOutput::NoView { reason, .. } => assert!(reason.contains("roof"), "got: {reason}"),
            other => panic!("expected NoView, got {other:?}"),
        }
    }
}
