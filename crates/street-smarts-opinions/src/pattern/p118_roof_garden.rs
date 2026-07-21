//! P118 Roof Garden — flat, usable sections of roof should be developed
//! as real gardens, reachable directly from a lived-in floor.
//!
//! From Alexander, *A Pattern Language*, Pattern 118 (p. 575), via
//! patternlanguage.cc/Patterns/Roof-Garden-(118):
//! > **Problem:** A vast part of the earth's surface, in a town,
//! > consists of roofs... it is natural, and indeed essential, to make
//! > roofs which take advantage of the sun and air.
//! > **Solution:** Make parts of almost every roof system usable as roof
//! > gardens... always make it possible to walk directly out onto the
//! > roof garden from some lived-in part of the building.
//!
//! # A real, honest gap -- not a proxy
//!
//! `p117_sheltering_roof` now assigns every real building a real
//! `Building.roof` (see its own module doc), but always a sloped
//! `RoofShape::Shed` -- never `Flat`. This pattern's own doc is explicit
//! that a real roof garden needs P116/P117 to land first (`RoofShape::
//! Flat` exists in the schema specifically reserved for this future use),
//! but a flat, walkable section isn't produced yet, and there's no
//! roof-access data (a real path from an interior cell out onto the
//! roof) either. This opinion exists, with a real citation, specifically
//! to be honest about that: it always returns `NoView` with the real
//! reason, rather than inventing a proxy that isn't real data.

use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P118RoofGarden;

impl Opinion for P118RoofGarden {
    fn name(&self) -> &'static str {
        "p118_roof_garden"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p118".into(),
            display: "Alexander et al., A Pattern Language, Pattern 118 (Roof Garden)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Roof-Garden-(118)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, _n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();
        OpinionOutput::NoView {
            reason: "p117_sheltering_roof only ever assigns a sloped Shed roof, never Flat, and \
                     there's no roof-access data -- nothing to check a real roof-garden claim \
                     against yet. See this opinion's own module doc."
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
                layer_provenance: Default::default(), label: "P118 unit fixture".into(),
            },
        };
        match P118RoofGarden.evaluate(&n) {
            OpinionOutput::NoView { reason, .. } => assert!(reason.contains("roof"), "got: {reason}"),
            other => panic!("expected NoView, got {other:?}"),
        }
    }
}
