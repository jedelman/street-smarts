//! P96 Number of Stories — turn a block's density-ring goal into real
//! per-building story counts, honoring P21's four-story-ordinary-building
//! constraint.
//!
//! From Alexander, *A Pattern Language*, Pattern 96:
//! > Assign each building its correct height... In any neighborhood, four
//! > stories should be treated as an absolute upper limit, with three
//! > stories more generally the rule -- except for a very few buildings,
//! > which are the exceptions, and which should be placed with great care.
//!
//! # What this does
//! Runs once, site-scale (`parcel_id == "*"`), over every parcel tagged
//! `use_category: "p95_building_pad"` (or `"p95_pad_with_building"`) --
//! the same convention P107 already uses. Groups pads by `density_tier`
//! (set by P29 Density Rings; pads with no tier fall into an
//! `"unspecified"` group using `default_target_stories`). Within each
//! group:
//! - Every pad is capped at `max_ordinary_stories` (Alexander's own
//!   number, default 4) UNLESS the group's `target_stories` (P29's goal)
//!   exceeds the cap.
//! - When it does, up to `tall_exception_fraction` of that group's pads
//!   (by count, rounded down, at least one if the group is non-empty) may
//!   be assigned the group's full `target_stories` instead of the cap --
//!   Alexander's "very few... exceptions." Candidates are picked
//!   largest-pad-first (a taller building deserves a correspondingly
//!   larger footprint, not a token exception on a sliver) and only kept if
//!   they're at least `min_tall_spacing_m` from every exception already
//!   chosen -- the "placed with great care... widely spaced" half of the
//!   pattern, not just a bare count limit.
//!
//! Sets `target_stories` on each pad's own `Parcel` (overwriting P29's
//! block-level goal with this pad's actual assignment). Does NOT set
//! `height_m` directly -- there's no such field on `Parcel`; P107 reads
//! `target_stories` back (via `floor_to_floor_m`) when it creates the real
//! `Building` entity. A pipeline that runs P96 before P107 gets real
//! height variation; skipping P96 leaves P107's own flat
//! `assumed_height_m` fallback unchanged, so this is backward compatible.
//!
//! # What this deliberately does NOT do
//! - Doesn't reconsider whether a pad's FOOTPRINT should change for a
//!   taller building (larger footprint for structural/egress reasons at
//!   height) -- purely assigns a story count to whatever footprint P95
//!   already produced.
//! - `min_tall_spacing_m` spacing is checked pad-centroid to pad-centroid,
//!   straight-line -- doesn't account for what's between them (a tall
//!   exception across a street from another one still counts as "close").
//!
//! # v0.2: P99 Main Building, one real extension
//!
//! Alexander's P99: "decide which building... houses the most essential
//! function... form this building as the main building, with a central
//! position, higher roof." This schema has no program/use data to identify
//! "most essential function" -- but position and height are both real.
//! After the per-tier story assignment above, when 2+ pads exist, the ONE
//! pad nearest the area-weighted centroid of every pad gets
//! `main_building_boost_stories` added on top of whatever its tier already
//! assigned -- uncapped by `max_ordinary_stories`, the same class of
//! deliberate override as a tall exception, since Alexander's own text
//! explicitly singles this one building out. This directly targets
//! `p99_main_building`'s own two real proxies: height_dominance (the boost
//! makes it measurably taller) and centrality (it's the nearest-to-center
//! pad by construction).

use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Neighborhood, Parcel};
use street_smarts_core::Scope;
use street_smarts_core::opinion::SourceCitation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P96Params {
    /// Alexander's own number: an absolute upper limit for ordinary
    /// buildings (P21 Four-Story Limit).
    pub max_ordinary_stories: f64,
    /// Fraction of a density tier's pads that may become "the exceptions"
    /// when that tier's target exceeds the ordinary cap.
    pub tall_exception_fraction: f64,
    /// Minimum straight-line distance required between two tall
    /// exceptions -- Alexander's "widely spaced," checked crudely.
    pub min_tall_spacing_m: f64,
    /// Target story count for pads with no `density_tier` (P29 didn't
    /// run, or ran with a different scope). Kept at the ordinary default
    /// so P96 alone (no P29) is a no-op relative to P107's own flat
    /// default height.
    pub default_target_stories: f64,
    /// Extra stories added, on top of its tier's normal assignment, to
    /// the ONE pad nearest the area-weighted centroid of every pad --
    /// Alexander's P99 Main Building ("form this building... with a
    /// central position, higher roof"). 0 = no main-building boost.
    pub main_building_boost_stories: f64,
}

impl Parameters for P96Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "max_ordinary_stories",
                "Absolute upper limit for ordinary buildings (P21 Four-Story Limit).",
                2.0, 8.0, 4.0,
            ).with_unit("stories"),
            ParamSpec::float(
                "tall_exception_fraction",
                "Fraction of a tier's pads allowed to exceed the ordinary cap, when the tier calls for it.",
                0.0, 0.5, 0.15,
            ),
            ParamSpec::float(
                "min_tall_spacing_m",
                "Minimum straight-line distance required between two tall exceptions.",
                20.0, 300.0, 80.0,
            ).with_unit("m"),
            ParamSpec::float(
                "default_target_stories",
                "Target stories for pads with no density_tier (P29 didn't run).",
                1.0, 8.0, 3.0,
            ).with_unit("stories"),
            ParamSpec::float(
                "main_building_boost_stories",
                "Extra stories added to the one pad nearest the site's own area-weighted centroid (P99 Main Building).",
                0.0, 10.0, 3.0,
            ).with_unit("stories"),
        ]
    }
    fn defaults() -> Self {
        Self {
            max_ordinary_stories: 4.0,
            tall_exception_fraction: 0.15,
            min_tall_spacing_m: 80.0,
            default_target_stories: 3.0,
            main_building_boost_stories: 3.0,
        }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.max_ordinary_stories, self.tall_exception_fraction, self.min_tall_spacing_m, self.default_target_stories, self.main_building_boost_stories]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.max_ordinary_stories = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.tall_exception_fraction = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) { p.min_tall_spacing_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(3), v.get(3)) { p.default_target_stories = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(4), v.get(4)) { p.main_building_boost_stories = s.clamp(*x); }
        p
    }
}

pub struct P96NumberOfStories;

fn pad_centroid(p: &Parcel) -> LngLat {
    LngLat::new(
        p.polygon.outer.iter().map(|q| q.lng).sum::<f64>() / p.polygon.outer.len().max(1) as f64,
        p.polygon.outer.iter().map(|q| q.lat).sum::<f64>() / p.polygon.outer.len().max(1) as f64,
    )
}

fn dist_m(a: LngLat, b: LngLat) -> f64 {
    let lat0 = (a.lat + b.lat) / 2.0;
    let dx = (a.lng - b.lng) * 111_320.0 * lat0.to_radians().cos();
    let dy = (a.lat - b.lat) * 110_540.0;
    (dx * dx + dy * dy).sqrt()
}

impl PatternOperator for P96NumberOfStories {
    type Params = P96Params;

    fn name(&self) -> &'static str { "p96_number_of_stories" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p96".into(),
            display: "Alexander et al., A Pattern Language, Pattern 96 (Number of Stories)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl96/apl96.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Assign each building pad a real story count from its density-ring goal, capping ordinary buildings at 4 stories and allowing a very few, widely-spaced exceptions."
    }

    /// `parcel_id == "*"` (the only mode supported): assigns every pad
    /// tagged `p95_building_pad`/`p95_pad_with_building` in one pass.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p96_number_of_stories only supports parcel_id \"*\" -- it assigns every building pad in one pass.".into());
        }

        let pads: Vec<&Parcel> = nbhd.select(&Scope::BUILDING_PAD).collect();
        if pads.is_empty() {
            return Err("p96_number_of_stories: no building pads found -- run P95 Building Complex first.".into());
        }

        // Group pad indices by tier, largest area first within each group
        // (a taller building gets a correspondingly larger footprint, not
        // a token exception on a sliver).
        let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, p) in pads.iter().enumerate() {
            let tier = p.density_tier.clone().unwrap_or_else(|| "unspecified".into());
            groups.entry(tier).or_default().push(i);
        }

        let mut assigned_stories: Vec<f64> = vec![params.default_target_stories; pads.len()];
        let mut n_tall = 0;
        let mut n_ordinary = 0;
        let mut steps: Vec<String> = Vec::new();

        for (tier, mut idxs) in groups {
            idxs.sort_by(|&a, &b| pads[b].polygon.area_m2().partial_cmp(&pads[a].polygon.area_m2()).unwrap_or(std::cmp::Ordering::Equal));

            let tier_target = if tier == "unspecified" {
                params.default_target_stories
            } else {
                idxs.iter().find_map(|&i| pads[i].target_stories).unwrap_or(params.default_target_stories)
            };

            if tier_target <= params.max_ordinary_stories {
                for &i in &idxs {
                    assigned_stories[i] = tier_target.max(1.0);
                    n_ordinary += 1;
                }
                steps.push(format!("{tier}: {} pad(s), all ordinary at {:.0} stories (tier target within the cap).", idxs.len(), tier_target));
                continue;
            }

            let max_exceptions = ((idxs.len() as f64) * params.tall_exception_fraction).floor().max(1.0) as usize;
            let mut chosen_tall: Vec<LngLat> = Vec::new();
            let mut n_tier_tall = 0;
            for &i in &idxs {
                if n_tier_tall >= max_exceptions {
                    assigned_stories[i] = params.max_ordinary_stories;
                    n_ordinary += 1;
                    continue;
                }
                let c = pad_centroid(pads[i]);
                let too_close = chosen_tall.iter().any(|&other| dist_m(c, other) < params.min_tall_spacing_m);
                if too_close {
                    assigned_stories[i] = params.max_ordinary_stories;
                    n_ordinary += 1;
                } else {
                    assigned_stories[i] = tier_target;
                    chosen_tall.push(c);
                    n_tier_tall += 1;
                    n_tall += 1;
                }
            }
            steps.push(format!(
                "{tier}: {} pad(s), {} tall exception(s) at {:.0} stories (spaced >= {:.0}m apart), {} ordinary at the {:.0}-story cap.",
                idxs.len(), n_tier_tall, tier_target, params.min_tall_spacing_m, idxs.len() - n_tier_tall, params.max_ordinary_stories
            ));
        }

        // P99 Main Building: the one pad nearest the area-weighted centroid
        // of every pad gets a real boost on top of its tier's assignment,
        // uncapped -- see this file's own "v0.2" module doc.
        let mut main_building_idx: Option<usize> = None;
        if pads.len() >= 2 && params.main_building_boost_stories > 0.0 {
            let mut cx = 0.0;
            let mut cy = 0.0;
            let mut total_area = 0.0;
            let centroids: Vec<LngLat> = pads.iter().map(|p| pad_centroid(p)).collect();
            for (p, c) in pads.iter().zip(&centroids) {
                let a = p.polygon.area_m2().max(1.0);
                cx += c.lng * a;
                cy += c.lat * a;
                total_area += a;
            }
            let center = LngLat::new(cx / total_area, cy / total_area);
            let (idx, _) = centroids.iter().enumerate()
                .map(|(i, c)| (i, dist_m(*c, center)))
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                .expect("pads is non-empty here");
            assigned_stories[idx] += params.main_building_boost_stories;
            main_building_idx = Some(idx);
        }

        let mut new_parcels: Vec<Parcel> = Vec::with_capacity(pads.len());
        let mut replaced: Vec<String> = Vec::with_capacity(pads.len());
        for (i, p) in pads.iter().enumerate() {
            let mut updated = (*p).clone();
            updated.target_stories = Some(assigned_stories[i]);
            new_parcels.push(updated);
            replaced.push(p.id.clone());
        }
        if let Some(idx) = main_building_idx {
            steps.push(format!(
                "Main building: {} boosted by {:.0} stories (now {:.0}) as the pad nearest the site's own centroid.",
                pads[idx].id, params.main_building_boost_stories, assigned_stories[idx]
            ));
        }

        let trace = SubdivisionTrace {
            operator_name: "p96_number_of_stories".into(),
            operator_source: self.source(),
            headline: format!(
                "Assigned real story counts to {} building pad(s): {} ordinary (<= {:.0} stories), {} tall exception(s).",
                pads.len(), n_ordinary, params.max_ordinary_stories, n_tall
            ),
            steps,
            caveats: vec![
                "Tall-exception spacing is checked pad-centroid to pad-centroid, straight-line -- \
                 doesn't account for what's actually between two pads (a street, another block); \
                 an exception across a street from another one still counts as 'too close' even \
                 though a real street is real separation, and vice versa for a diagonal gap with \
                 nothing between them.".into(),
                "Assigns a story count to whatever footprint P95 already produced -- doesn't \
                 reconsider whether a taller building needs a larger footprint for structure or \
                 egress.".into(),
                "Pads with no density_tier (P29 didn't run, or ran on a different scope) all get \
                 the same flat default_target_stories -- P96 alone, without P29, doesn't create \
                 any real height variation.".into(),
                "The main-building pad is picked purely by position (nearest the pads' own \
                 area-weighted centroid) -- this schema has no 'most essential function'/program \
                 data to pick it by use, as Alexander's own text actually prescribes.".into(),
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
            replaced_parcel_ids: replaced,
            replaced_open_space_ids: vec![],
            replaced_building_ids: vec![],
            entity_provenance: std::collections::BTreeMap::new(),
            trace,
        })
    }
}
