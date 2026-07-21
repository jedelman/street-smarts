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
//! `p117_sheltering_roof` now assigns every real building a real
//! `Building.roof` (a shed roof, see its own module doc), so this
//! pipeline is no longer entirely without roof geometry -- but that
//! operator deliberately assigns ONE roof segment per whole building,
//! not the per-WING cascade this pattern specifically asks for ("the
//! floors step down toward the ends of wings, and... the roof,
//! accordingly, forms a cascade"). Checking THIS claim for real would
//! need `RoofForm` to carry per-wing segments keyed to
//! `p127_intimacy_gradient`'s own cell graph, which doesn't exist yet --
//! a real, separate, larger lift, not something P117's own single-segment
//! roof can stand in for. This opinion exists, with a real citation,
//! specifically to be honest about that: it always returns `NoView` with
//! the real reason, rather than inventing a proxy that isn't real data.

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
            reason: "p117_sheltering_roof now assigns every building ONE real roof segment, not the \
                     real per-wing cascade this pattern needs -- RoofForm has no per-wing segment \
                     field yet. See this opinion's own module doc."
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
