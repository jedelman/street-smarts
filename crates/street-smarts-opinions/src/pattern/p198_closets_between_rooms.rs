//! P198 Closets Between Rooms — storage should be placed on interior
//! walls between rooms, doing double duty as acoustic insulation and
//! entry transitions, never on exterior walls.
//!
//! From Alexander, *A Pattern Language*, Pattern 198 (p. 913), via
//! patternlanguage.cc/Patterns/Closets-Between-Rooms-(198):
//! > **Problem:** The provision of storage and closets usually comes as
//! > an afterthought.
//! > **Solution:** Mark all the rooms where you want closets. Then place
//! > the closets themselves on those interior walls which lie between two
//! > rooms and between rooms and passages where you need acoustic
//! > insulation. Place them so as to create transition spaces for the
//! > doors into the rooms. On no account put closets on exterior walls.
//!
//! # A real, honest gap -- not a proxy
//!
//! No closet or storage concept exists anywhere in this pipeline's
//! schema. `InteriorCell.kind` only ever takes the values `"room"`,
//! `"passage"`, or (since `p133_staircase_as_a_stage`) `"stair"` -- there
//! is no cell type, or any other field, representing a storage closet
//! carved into an interior wall. A small-area/low-connectivity cell would
//! not really BE a closet; it would be an arbitrary size threshold on a
//! room cell wearing a closet's name. This opinion exists, with a real
//! citation, specifically to be honest about that: it always returns
//! `NoView` with the real reason, rather than inventing a proxy that
//! isn't real data.

use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P198ClosetsBetweenRooms;

impl Opinion for P198ClosetsBetweenRooms {
    fn name(&self) -> &'static str {
        "p198_closets_between_rooms"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p198".into(),
            display: "Alexander et al., A Pattern Language, Pattern 198 (Closets Between Rooms)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Closets-Between-Rooms-(198)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, _n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();
        OpinionOutput::NoView {
            reason: "No closet/storage concept exists anywhere in this pipeline's schema -- \
                     InteriorCell.kind only ever takes \"room\", \"passage\", or \"stair\". See this \
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
                layer_provenance: Default::default(), label: "P198 unit fixture".into(),
            },
        };
        match P198ClosetsBetweenRooms.evaluate(&n) {
            OpinionOutput::NoView { reason, .. } => assert!(reason.contains("closet")),
            other => panic!("expected NoView, got {other:?}"),
        }
    }
}
