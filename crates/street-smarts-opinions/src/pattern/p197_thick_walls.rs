//! P197 Thick Walls — walls should occupy real volume, not read as thin,
//! prefabricated membranes with no depth.
//!
//! From Alexander, *A Pattern Language*, Pattern 197 (p. 908), via
//! patternlanguage.cc/Patterns/Thick-Walls-(197):
//! > **Problem:** Houses with smooth hard walls made of prefabricated
//! > panels, concrete, gypsum, steel, aluminum, or glass always stay
//! > impersonal and dead.
//! > **Solution:** Open your mind to the possibility that the walls of
//! > your building can be thick, can occupy a substantial volume -- even
//! > actual usable space -- and need not be merely thin membranes which
//! > have no depth.
//!
//! # A real, honest gap -- not a proxy
//!
//! `render.py`'s own documented caveat applies directly here: a punch just
//! pierces solid mass -- no wall-thickness field exists anywhere in this
//! pipeline's schema. `Building` models an outer footprint ring and
//! `Opening`s, with nothing separating a wall's interior and exterior
//! faces. This opinion exists, with a real citation, specifically to be
//! honest about that: it always returns `NoView` with the real reason,
//! rather than inventing a proxy that isn't real data.

use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P197ThickWalls;

impl Opinion for P197ThickWalls {
    fn name(&self) -> &'static str {
        "p197_thick_walls"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p197".into(),
            display: "Alexander et al., A Pattern Language, Pattern 197 (Thick Walls)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Thick-Walls-(197)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, _n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();
        OpinionOutput::NoView {
            reason: "No wall-thickness field exists anywhere in this pipeline's schema -- a \
                     Building's outer ring and its Openings model a zero-depth membrane, matching \
                     render.py's own documented caveat that a punch just pierces solid mass. See \
                     this opinion's own module doc."
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
                layer_provenance: Default::default(), label: "P197 unit fixture".into(),
            },
        };
        match P197ThickWalls.evaluate(&n) {
            OpinionOutput::NoView { reason, .. } => assert!(reason.contains("thickness")),
            other => panic!("expected NoView, got {other:?}"),
        }
    }
}
