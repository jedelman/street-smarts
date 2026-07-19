//! P116 Cascade of Roofs — roofs should step down toward wing ends,
//! following the social hierarchy of the spaces below.
//!
//! From Alexander, *A Pattern Language*, Pattern 116 (p. 565), via
//! patternlanguage.cc/Patterns/Cascade-of-Roofs-(116):
//! > **Problem:** Few buildings will be structurally and socially
//! > intact, unless the floors step down toward the ends of wings, and
//! > unless the roof, accordingly, forms a cascade.
//! > **Solution:** Designers should envision the entire building as a
//! > roof system, positioning the largest and highest roofs over the
//! > most significant areas. Lesser roofs should cascade from these
//! > primary structures.
//!
//! # A real, honest gap -- not a proxy
//!
//! Every claim here is about roof FORM (pitch, height, cascading) --
//! and this pipeline models no roof geometry at all. Every building is a
//! flat-topped extrusion (see `tools/vibe-render/render.py`'s own
//! explicit "no roof forms" caveat); `Building` has no roof-shape, roof-
//! pitch, or roof-height field to check against. This opinion exists,
//! with a real citation, specifically to be honest about that: it always
//! returns `NoView` with the real reason, rather than inventing a proxy
//! that isn't real data.

use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P116CascadeOfRoofs;

impl Opinion for P116CascadeOfRoofs {
    fn name(&self) -> &'static str {
        "p116_cascade_of_roofs"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p116".into(),
            display: "Alexander et al., A Pattern Language, Pattern 116 (Cascade of Roofs)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Cascade-of-Roofs-(116)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, _n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();
        OpinionOutput::NoView {
            reason: "This pipeline models no roof geometry at all -- every building is a flat-topped \
                     extrusion, with no roof-shape, pitch, or height field to check a cascade claim \
                     against. See this opinion's own module doc."
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
                layer_provenance: Default::default(), label: "P116 unit fixture".into(),
            },
        };
        match P116CascadeOfRoofs.evaluate(&n) {
            OpinionOutput::NoView { reason, .. } => assert!(reason.contains("roof"), "got: {reason}"),
            other => panic!("expected NoView, got {other:?}"),
        }
    }
}
