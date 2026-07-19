//! P166 Gallery Surround — porches, balconies, and arcades at a
//! building's edge should let people step out into the public world at
//! every story.
//!
//! From Alexander, *A Pattern Language*, Pattern 166 (p. 777), via
//! patternlanguage.cc/Patterns/Gallery-Surround-(166):
//! > **Problem:** If people cannot walk out from the building onto
//! > balconies and terraces which look toward the outdoor space around
//! > the building, then neither they themselves nor the people outside
//! > have any medium which helps them feel the building and the larger
//! > public world are intertwined.
//! > **Solution:** Whenever possible, and at every story, build porches,
//! > galleries, arcades, balconies, niches, outdoor seats, awnings,
//! > trellised rooms.
//!
//! # A real, honest gap -- not a proxy
//!
//! Same real gap as `p119_arcades.rs`: none of porches, balconies,
//! galleries, or awnings exist anywhere in this pipeline's schema --
//! `Building` models walls and openings only, nothing projecting from a
//! wall's edge at any story. This opinion exists, with a real citation,
//! specifically to be honest about that: it always returns `NoView` with
//! the real reason, rather than inventing a proxy that isn't real data.

use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P166GallerySurround;

impl Opinion for P166GallerySurround {
    fn name(&self) -> &'static str {
        "p166_gallery_surround"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p166".into(),
            display: "Alexander et al., A Pattern Language, Pattern 166 (Gallery Surround)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Gallery-Surround-(166)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, _n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();
        OpinionOutput::NoView {
            reason: "No porch/balcony/gallery/awning concept exists anywhere in this pipeline's \
                     schema -- Building only models walls and openings. See this opinion's own \
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
                layer_provenance: Default::default(), label: "P166 unit fixture".into(),
            },
        };
        match P166GallerySurround.evaluate(&n) {
            OpinionOutput::NoView { reason, .. } => assert!(reason.contains("porch") || reason.contains("balcony") || reason.contains("gallery"), "got: {reason}"),
            other => panic!("expected NoView, got {other:?}"),
        }
    }
}
