//! P162 North Face — the north side of a building should cascade down to
//! the ground so the sun reaches beside it, instead of casting a dead,
//! shadowed strip.
//!
//! From Alexander, *A Pattern Language*, Pattern 162 (p. 761), via
//! patternlanguage.cc/Patterns/North-Face-(162):
//! > **Problem:** Look at the north sides of the buildings which you
//! > know. Almost everywhere you will find that these are the spots
//! > which are dead and dank, gloomy and useless.
//! > **Solution:** Make the north face of the building a cascade which
//! > slopes down to the ground.
//!
//! # A real, honest gap -- not a proxy
//!
//! Same real gap as `p116_cascade_of_roofs.rs`: a north-face cascade is
//! a roof/massing-profile claim, and this pipeline models no roof
//! geometry at all -- every building is a flat-topped extrusion with no
//! slope or profile-by-facade data. This opinion exists, with a real
//! citation, specifically to be honest about that: it always returns
//! `NoView` with the real reason, rather than inventing a proxy that
//! isn't real data.

use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P162NorthFace;

impl Opinion for P162NorthFace {
    fn name(&self) -> &'static str {
        "p162_north_face"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p162".into(),
            display: "Alexander et al., A Pattern Language, Pattern 162 (North Face)".into(),
            url: Some("https://patternlanguage.cc/Patterns/North-Face-(162)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, _n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();
        OpinionOutput::NoView {
            reason: "This pipeline models no roof/massing-profile geometry -- every building is a \
                     flat-topped extrusion with no facade-by-facade slope data to check a north-face \
                     cascade claim against. See this opinion's own module doc."
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
                layer_provenance: Default::default(), label: "P162 unit fixture".into(),
            },
        };
        match P162NorthFace.evaluate(&n) {
            OpinionOutput::NoView { reason, .. } => assert!(reason.contains("roof") || reason.contains("facade"), "got: {reason}"),
            other => panic!("expected NoView, got {other:?}"),
        }
    }
}
