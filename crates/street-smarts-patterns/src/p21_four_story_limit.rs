//! P21 Four-Story Limit — cap every building pad's aspirational story
//! count at Alexander's own ordinary ceiling, the moment a pad has one to
//! cap.
//!
//! From Alexander, *A Pattern Language*, Pattern 21:
//! > In any given region, note the height of traditional buildings... In
//! > any neighborhood, four stories should be treated as an absolute upper
//! > limit, with three stories more generally the rule -- except for a very
//! > few buildings, which are the exceptions, and which should be placed
//! > with great care.
//!
//! # Where this fits: splitting P96, closing PATTERN_ORDERING_AUDIT.md §4.2
//!
//! `p96_number_of_stories.rs` used to do two genuinely different jobs in
//! one pass, both running after P108 Connected Buildings purely because
//! that's where the single combined operator happened to sit: (1) capping
//! every pad's tier-inherited target at the ordinary ceiling, and (2)
//! picking which very few pads get to exceed it, "placed with great
//! care... widely spaced." Only job (2) genuinely needs P108's FINAL,
//! merged pad set (final areas for largest-first exception ranking, final
//! positions for the spacing check). Job (1) is a pure per-pad read of
//! whatever `target_stories` P29/P37's field sampling already put on this
//! specific pad -- it doesn't need P108, P96's own tier grouping, or
//! anything about any OTHER pad. That's this operator's own real
//! Alexander citation (P21 -- already named in P96's own module doc and
//! this crate's `street-smarts-opinions::p21_four_story_limit` detector,
//! just not previously given a generator of its own), so it gets to run
//! at its own real earliest position: right after P95 creates pads,
//! before P108 merges any of them.
//!
//! # What this does
//! Runs once, site-scale (`parcel_id == "*"`), over every parcel tagged
//! `use_category: "p95_building_pad"`. For each pad: `raw =
//! target_stories` (P29/P37's field-sampled goal for this pad, or
//! `default_target_stories` if P29 never ran) capped at
//! `max_ordinary_stories`. Writes the capped value back onto
//! `target_stories` -- P96 (now exceptions-only, after P108) either
//! leaves this value alone or overwrites it for the very few pads it picks
//! as real exceptions.
//!
//! # What this deliberately does NOT do
//! - Doesn't pick exceptions -- that's real individuation work that needs
//!   the final, merged pad set (P108's job to produce), left entirely to
//!   `p96_number_of_stories`.
//! - Doesn't reconsider `density_tier` or any other field -- pure
//!   read-cap-write on `target_stories` alone.

use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::nir::{Neighborhood, Parcel};
use street_smarts_core::Scope;
use street_smarts_core::opinion::SourceCitation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P21Params {
    /// Alexander's own number: an absolute upper limit for ordinary
    /// buildings.
    pub max_ordinary_stories: f64,
    /// Target stories for a pad with no `target_stories` yet (P29 didn't
    /// run) -- kept at the ordinary default so this operator alone, without
    /// P29, is a no-op relative to P107's own flat default height.
    pub default_target_stories: f64,
}

impl Parameters for P21Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "max_ordinary_stories",
                "Absolute upper limit for ordinary buildings.",
                2.0, 8.0, 4.0,
            ).with_unit("stories"),
            ParamSpec::float(
                "default_target_stories",
                "Target stories for pads with no target_stories yet (P29 didn't run).",
                1.0, 8.0, 3.0,
            ).with_unit("stories"),
        ]
    }
    fn defaults() -> Self {
        Self { max_ordinary_stories: 4.0, default_target_stories: 3.0 }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.max_ordinary_stories, self.default_target_stories]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.max_ordinary_stories = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.default_target_stories = s.clamp(*x); }
        p
    }
}

pub struct P21FourStoryLimit;

impl PatternOperator for P21FourStoryLimit {
    type Params = P21Params;

    fn name(&self) -> &'static str { "p21_four_story_limit" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p21".into(),
            display: "Alexander et al., A Pattern Language, Pattern 21 (Four-Story Limit)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl21/apl21.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Cap every building pad's field-inherited story target at the ordinary ceiling, before P108 merges any pads."
    }

    /// `parcel_id == "*"` (the only mode supported): caps every pad tagged
    /// `p95_building_pad` in one pass.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p21_four_story_limit only supports parcel_id \"*\" -- it caps every building pad in one pass.".into());
        }

        let pads: Vec<&Parcel> = nbhd.select(&Scope::BUILDING_PAD).collect();
        if pads.is_empty() {
            return Err("p21_four_story_limit: no building pads found -- run P95 Building Complex first.".into());
        }

        let mut new_parcels: Vec<Parcel> = Vec::with_capacity(pads.len());
        let mut replaced: Vec<String> = Vec::with_capacity(pads.len());
        let mut n_capped = 0;

        for p in &pads {
            let raw = p.target_stories.unwrap_or(params.default_target_stories);
            let capped = raw.min(params.max_ordinary_stories).max(1.0);
            if capped < raw - 1e-9 {
                n_capped += 1;
            }
            let mut updated = (*p).clone();
            updated.target_stories = Some(capped);
            new_parcels.push(updated);
            replaced.push(p.id.clone());
        }

        let trace = SubdivisionTrace {
            operator_name: "p21_four_story_limit".into(),
            operator_source: self.source(),
            headline: format!(
                "Capped {} building pad(s) at {:.0} ordinary stories; {} pad(s) had a field-sampled goal above the cap (left at the cap here -- p96_number_of_stories picks the very few real exceptions after P108 merges pads).",
                pads.len(), params.max_ordinary_stories, n_capped
            ),
            steps: vec![format!(
                "{} of {} pad(s) capped from a higher field-sampled target down to {:.0} stories.",
                n_capped, pads.len(), params.max_ordinary_stories
            )],
            caveats: vec![
                "Doesn't pick exceptions -- every over-cap pad is left at the ordinary cap here; \
                 p96_number_of_stories (after P108) promotes a very few of them back up, using \
                 P108's final merged footprint to rank and space them.".into(),
                "Pads with no target_stories yet (P29 didn't run) all get the same flat \
                 default_target_stories -- this operator alone, without P29, creates no real \
                 height variation.".into(),
            ],
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels,
            new_open_space: vec![],
            new_buildings: vec![],
            new_streets: vec![],
            new_activity_nodes: vec![],
            new_boundaries: vec![],
            replaced_parcel_ids: replaced,
            replaced_open_space_ids: vec![],
            replaced_building_ids: vec![],
            entity_provenance: std::collections::BTreeMap::new(),
            trace,
            new_fields: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::NeighborhoodMeta;

    fn pad(id: &str, target_stories: Option<f64>) -> Parcel {
        Parcel {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0), LngLat::new(0.001, 0.0),
                LngLat::new(0.001, 0.001), LngLat::new(0.0, 0.001), LngLat::new(0.0, 0.0),
            ]),
            area_acres: 1.0,
            use_category: Some("p95_building_pad".into()),
            ownership: None,
            is_eda: true,
            spec: None,
            density_tier: None,
            target_stories,
        }
    }

    fn nbhd(parcels: Vec<Parcel>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
            parcels, buildings: vec![], streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![], pattern_fields: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P21 unit fixture".into(),
            },
        }
    }

    #[test]
    fn wildcard_only_mode() {
        let n = nbhd(vec![pad("A", Some(3.0))]);
        assert!(P21FourStoryLimit.apply(&n, "A", &P21Params::defaults(), 0).is_err());
    }

    #[test]
    fn no_pads_is_an_error() {
        let n = nbhd(vec![]);
        assert!(P21FourStoryLimit.apply(&n, "*", &P21Params::defaults(), 0).is_err());
    }

    #[test]
    fn a_pad_under_the_cap_is_left_at_its_own_target() {
        let n = nbhd(vec![pad("A", Some(3.0))]);
        let sub = P21FourStoryLimit.apply(&n, "*", &P21Params::defaults(), 0).unwrap();
        assert_eq!(sub.new_parcels[0].target_stories, Some(3.0));
    }

    #[test]
    fn a_pad_over_the_cap_is_capped_down() {
        let n = nbhd(vec![pad("A", Some(8.0))]);
        let sub = P21FourStoryLimit.apply(&n, "*", &P21Params::defaults(), 0).unwrap();
        assert_eq!(sub.new_parcels[0].target_stories, Some(4.0));
    }

    #[test]
    fn a_pad_with_no_target_stories_gets_the_default() {
        let n = nbhd(vec![pad("A", None)]);
        let sub = P21FourStoryLimit.apply(&n, "*", &P21Params::defaults(), 0).unwrap();
        assert_eq!(sub.new_parcels[0].target_stories, Some(3.0));
    }
}
