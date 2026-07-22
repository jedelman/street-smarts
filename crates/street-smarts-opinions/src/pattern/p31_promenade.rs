//! P31 Promenade — a community needs a central, linear public-life spine
//! linking its main activity nodes, within a 10-minute walk of every
//! point.
//!
//! From Alexander, *A Pattern Language*, Pattern 31 (p. 168), via
//! patternlanguage.cc/Patterns/Promenade-(31):
//! > **Problem:** Each subculture needs a center for its public life: a
//! > place where you can go to see people, and to be seen.
//! > **Solution:** Encourage the gradual formation of a promenade at the
//! > heart of every community, linking the main activity nodes, and
//! > placed centrally, so that each point in the community is within 10
//! > minutes' walk of it. Put main points of attraction at the two ends,
//! > to keep a constant movement up and down.
//!
//! # A real, honest gap -- not a proxy
//!
//! A promenade is a specific, named linear path threading a community's
//! main activity nodes -- nothing like it exists in this pipeline's
//! schema. `Neighborhood.activity_nodes: Vec<ActivityNode>` is a real,
//! typed field (see `p126_something_roughly_in_the_middle.rs`), but no
//! generator anywhere populates it, and even if it were populated there
//! is no "promenade" path type distinct from `Street.classification`'s
//! four values ("arterial"/"local"/"alley"/"pedestrian") to check a real
//! path against it. This opinion exists, with a real citation,
//! specifically to be honest about that: it always returns `NoView` with
//! the real reason, rather than inventing a proxy that isn't real data.

use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P31Promenade;

impl Opinion for P31Promenade {
    fn name(&self) -> &'static str {
        "p31_promenade"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p31".into(),
            display: "Alexander et al., A Pattern Language, Pattern 31 (Promenade)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Promenade-(31)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, _n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();
        OpinionOutput::NoView {
            reason: "No 'promenade' path concept exists in this pipeline's schema, and no \
                     generator populates Neighborhood.activity_nodes -- there is nothing to check a \
                     central, activity-node-linking path against. See this opinion's own module doc."
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
                layer_provenance: Default::default(), label: "P31 unit fixture".into(),
            },
            pattern_fields: vec![],
        };
        match P31Promenade.evaluate(&n) {
            OpinionOutput::NoView { reason, .. } => assert!(reason.contains("promenade")),
            other => panic!("expected NoView, got {other:?}"),
        }
    }
}
