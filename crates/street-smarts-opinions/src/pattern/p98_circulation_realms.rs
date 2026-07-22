//! P98 Circulation Realms — large buildings and building complexes should
//! be organized as a nested sequence of realms, each marked by a gateway
//! and growing smaller as one moves inward.
//!
//! From Alexander, *A Pattern Language*, Pattern 98 (p. 480), via
//! patternlanguage.cc/Patterns/Circulation-Realms-(98):
//! > **Problem:** In many modern building complexes the problem of
//! > disorientation is acute. People have no idea where they are, and
//! > they experience considerable mental stress as a result.
//! > **Solution:** Lay out very large buildings and collections of small
//! > buildings so that one reaches a given point inside by passing
//! > through a sequence of realms, each marked by a gateway and becoming
//! > smaller and smaller, as one passes from each one, through a gateway,
//! > to the next.
//!
//! # A real, honest gap -- not a proxy
//!
//! A nested realm-and-gateway hierarchy is a distinct organizational
//! concept from anything this schema models. `InteriorCell.depth`
//! (`p127_intimacy_gradient`) is a real public-to-private gradient, but
//! it is a single scalar per cell, not a sequence of marked, progressively
//! smaller realms each entered through a real gateway -- and no gateway
//! concept exists at all (see `p53_main_gateways.rs`'s own module doc on
//! the closest real field, `Neighborhood.boundaries`, which marks
//! neighborhood-scale edges, not interior circulation thresholds). This
//! opinion exists, with a real citation, specifically to be honest about
//! that: it always returns `NoView` with the real reason, rather than
//! inventing a proxy that isn't real data.

use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P98CirculationRealms;

impl Opinion for P98CirculationRealms {
    fn name(&self) -> &'static str {
        "p98_circulation_realms"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p98".into(),
            display: "Alexander et al., A Pattern Language, Pattern 98 (Circulation Realms)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Circulation-Realms-(98)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, _n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();
        OpinionOutput::NoView {
            reason: "No nested realm-and-gateway hierarchy concept exists anywhere in this \
                     pipeline's schema -- InteriorCell.depth is a single public-to-private scalar, \
                     not a sequence of marked, progressively smaller realms, and no interior gateway \
                     concept exists at all. See this opinion's own module doc."
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
                layer_provenance: Default::default(), label: "P98 unit fixture".into(),
            },
            pattern_fields: vec![],
        };
        match P98CirculationRealms.evaluate(&n) {
            OpinionOutput::NoView { reason, .. } => assert!(reason.contains("realm")),
            other => panic!("expected NoView, got {other:?}"),
        }
    }
}
