//! P21 Four-Story Limit — scores whether the built neighborhood actually
//! honors Alexander's height constraint, rather than trusting an operator's
//! own parameters to have enforced it.
//!
//! From Alexander, *A Pattern Language*, Pattern 21:
//! > In any given region, note the height of traditional buildings... In
//! > any neighborhood, four stories should be treated as an absolute upper
//! > limit, with three stories more generally the rule -- except for a very
//! > few buildings, which are the exceptions, and which should be placed
//! > with great care.
//!
//! Two independent checks, not one blended guess:
//!
//! - `ordinary_compliance`: area-weighted fraction of building floor area
//!   in buildings at or below `max_ordinary_stories` (4, Alexander's own
//!   number). Weighted by footprint area rather than a bare building
//!   count, so one huge tall building doesn't get diluted to
//!   insignificance by a hundred small compliant ones, or vice versa.
//! - `exception_spacing`: for the buildings that exceed the cap (the "very
//!   few exceptions"), the fraction of pairs that are at least
//!   `min_spacing_m` apart, centroid to centroid. Alexander's own text
//!   ties the exception allowance directly to spacing ("placed with great
//!   care... widely spaced") -- a pattern that lets height run wild in one
//!   cluster of towers isn't honoring this pattern just because the overall
//!   count is small. A neighborhood with 0 or 1 exception buildings scores
//!   this sub-score 1.0 (nothing to space badly).
//!
//! `value = 0.6 * ordinary_compliance + 0.4 * exception_spacing` -- weighted
//! toward compliance since that's the pattern's primary claim ("four
//! stories... an absolute upper limit"); spacing is the secondary
//! "placed with care" qualifier on the exceptions Alexander explicitly
//! allows.
//!
//! Story count is derived from `Building.height_m` using a fixed
//! `floor_to_floor_m` assumption (3.5m) -- the NIR schema has no explicit
//! floor/story field. Same convention `p107_wings_of_light` (the
//! `street-smarts-patterns` operator that actually converts a story count
//! into real height, via its own `floor_to_floor_m` param) uses in the
//! other direction -- `p96_number_of_stories` only ASSIGNS the per-pad
//! story count, it never touches height itself (see its own module doc).

use std::collections::BTreeMap;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Building, Neighborhood};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P21FourStoryLimit;

const MAX_ORDINARY_STORIES: f64 = 4.0;
const FLOOR_TO_FLOOR_M: f64 = 3.5;
const MIN_SPACING_M: f64 = 80.0;

/// Real buildings have a whole number of stories -- a few centimeters of
/// per-building height jitter (organic variety, not a design decision) is
/// cosmetic, not an extra story. Round to the nearest whole story before
/// comparing against the cap, rather than treating height as a continuous
/// ratio -- otherwise a building an operator intentionally capped at
/// exactly 4 stories reads as "4.05 stories" and fails compliance purely
/// because its floor-to-floor height happened to jitter upward.
fn stories(b: &Building) -> f64 {
    (b.height_m.unwrap_or(FLOOR_TO_FLOOR_M * 3.0) / FLOOR_TO_FLOOR_M).round().max(1.0)
}

fn centroid(b: &Building) -> LngLat {
    let ring = &b.polygon.outer;
    LngLat::new(
        ring.iter().map(|p| p.lng).sum::<f64>() / ring.len().max(1) as f64,
        ring.iter().map(|p| p.lat).sum::<f64>() / ring.len().max(1) as f64,
    )
}

fn dist_m(a: LngLat, b: LngLat) -> f64 {
    let lat0 = (a.lat + b.lat) / 2.0;
    let dx = (a.lng - b.lng) * 111_320.0 * lat0.to_radians().cos();
    let dy = (a.lat - b.lat) * 110_540.0;
    (dx * dx + dy * dy).sqrt()
}

impl Opinion for P21FourStoryLimit {
    fn name(&self) -> &'static str {
        "p21_four_story_limit"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p21".into(),
            display: "Alexander et al., A Pattern Language, Pattern 21 (Four-Story Limit)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl21/apl21.htm".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        if n.buildings.is_empty() {
            return OpinionOutput::NoView {
                reason: "No buildings in this neighborhood -- nothing to check against the four-story limit.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut total_area = 0.0;
        let mut compliant_area = 0.0;
        let mut exceptions: Vec<&Building> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for b in &n.buildings {
            let footprint = b.polygon.area_m2();
            if footprint <= 0.0 {
                continue;
            }
            total_area += footprint;
            let s = stories(b);
            if s <= MAX_ORDINARY_STORIES {
                compliant_area += footprint;
            } else {
                exceptions.push(b);
            }
            details.insert(format!("{}.stories", b.id), format!("{:.1}", s));
        }

        if total_area <= 0.0 {
            return OpinionOutput::NoView {
                reason: "Buildings present but none had real footprint area to score.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let ordinary_compliance = compliant_area / total_area;

        let exception_spacing = if exceptions.len() < 2 {
            1.0
        } else {
            let centroids: Vec<LngLat> = exceptions.iter().map(|b| centroid(b)).collect();
            let mut n_pairs = 0;
            let mut n_well_spaced = 0;
            for i in 0..centroids.len() {
                for j in (i + 1)..centroids.len() {
                    n_pairs += 1;
                    if dist_m(centroids[i], centroids[j]) >= MIN_SPACING_M {
                        n_well_spaced += 1;
                    }
                }
            }
            n_well_spaced as f64 / n_pairs.max(1) as f64
        };

        let value = 0.6 * ordinary_compliance + 0.4 * exception_spacing;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("ordinary_compliance".into(), ordinary_compliance);
        sub_scores.insert("exception_spacing".into(), exception_spacing);

        details.insert("total_footprint_m2".into(), format!("{:.0}", total_area));
        details.insert("n_buildings".into(), n.buildings.len().to_string());
        details.insert("n_exceptions_over_4_stories".into(), exceptions.len().to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} building(s), {:.0}% of floor area at or below {:.0} stories. {} exception(s) over the cap, {:.0}% of exception pairs >= {:.0}m apart.",
                n.buildings.len(),
                ordinary_compliance * 100.0,
                MAX_ORDINARY_STORIES,
                exceptions.len(),
                exception_spacing * 100.0,
                MIN_SPACING_M
            ),
            sub_scores,
            details,
            caveats: vec![
                "Story count is derived from height_m / 3.5m -- the NIR schema has no explicit \
                 floor/story field. A building whose real story heights differ from that \
                 assumption (taller ground floor, mezzanines) will be misjudged.".into(),
                "exception_spacing checks straight-line centroid distance, not real separation -- \
                 doesn't know whether a street, a park, or nothing at all sits between two tall \
                 buildings.".into(),
                "ordinary_compliance is area-weighted across ALL buildings, including any that \
                 pattern operators never touched (e.g. hand-authored fixture data) -- this opinion \
                 can't distinguish 'this pipeline enforced the limit' from 'this data just happens \
                 to be low-rise.'".into(),
            ],
            contributing_features: exceptions.iter().map(|b| b.id.clone()).collect(),
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::NeighborhoodMeta;

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![],
            buildings,
            streets: vec![],
            open_space: vec![],
            boundaries: vec![],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P21 unit fixture".into(),
            },
        }
    }

    fn square_building(id: &str, lng: f64, lat: f64, side_deg: f64, height_m: f64) -> Building {
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(lng, lat),
                LngLat::new(lng + side_deg, lat),
                LngLat::new(lng + side_deg, lat + side_deg),
                LngLat::new(lng, lat + side_deg),
            ]),
            height_m: Some(height_m),
            typology: None,
            year_built: None,
            parcel_id: None,
            floors: None,
            openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        }
    }

    #[test]
    fn no_buildings_is_no_view() {
        let n = nbhd(vec![]);
        let out = P21FourStoryLimit.evaluate(&n);
        assert!(matches!(out, OpinionOutput::NoView { .. }));
    }

    #[test]
    fn all_buildings_under_the_cap_scores_perfectly() {
        let n = nbhd(vec![
            square_building("A", 0.0, 0.0, 0.0005, 3.5 * 3.0),
            square_building("B", 0.001, 0.0, 0.0005, 3.5 * 4.0),
        ]);
        let out = P21FourStoryLimit.evaluate(&n);
        match out {
            OpinionOutput::Value { value, sub_scores, .. } => {
                assert!((sub_scores["ordinary_compliance"] - 1.0).abs() < 1e-6);
                assert!((sub_scores["exception_spacing"] - 1.0).abs() < 1e-6, "0 exceptions should score spacing 1.0 (nothing to space badly)");
                assert!(value > 0.99);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_capped_four_story_building_with_jitter_noise_still_counts_as_compliant() {
        // Real pipeline behavior: P96 caps ordinary buildings at exactly 4
        // stories (14.0m at 3.5m/floor), then P107 adds small cosmetic
        // height jitter -- real observed range was ~13.7m to ~14.35m. None
        // of that should read as "5 stories."
        let n = nbhd(vec![
            square_building("JITTERED_UP", 0.0, 0.0, 0.0005, 14.34),
            square_building("JITTERED_DOWN", 0.001, 0.0, 0.0005, 13.66),
        ]);
        let out = P21FourStoryLimit.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, contributing_features, .. } => {
                assert!((sub_scores["ordinary_compliance"] - 1.0).abs() < 1e-6, "jitter around the 4-story cap should still count as compliant, got {}", sub_scores["ordinary_compliance"]);
                assert!(contributing_features.is_empty(), "neither building should be flagged as an exception");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn two_close_towers_score_low_on_spacing_even_if_area_is_small() {
        let n = nbhd(vec![
            square_building("ORDINARY", 0.0, 0.0, 0.001, 3.5 * 4.0),
            square_building("TOWER_A", 0.005, 0.005, 0.0001, 3.5 * 8.0),
            square_building("TOWER_B", 0.005005, 0.005, 0.0001, 3.5 * 8.0), // ~0.5m from TOWER_A -- well under the 80m threshold
        ]);
        let out = P21FourStoryLimit.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, contributing_features, .. } => {
                assert_eq!(contributing_features.len(), 2, "both towers should be flagged as exceptions");
                assert!((sub_scores["exception_spacing"]).abs() < 1e-6, "two towers placed right next to each other should score 0 on spacing");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn widely_spaced_towers_score_well_on_spacing() {
        let n = nbhd(vec![
            square_building("TOWER_A", 0.0, 0.0, 0.0001, 3.5 * 8.0),
            square_building("TOWER_B", 0.01, 0.01, 0.0001, 3.5 * 8.0), // ~1.5km away
        ]);
        let out = P21FourStoryLimit.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["exception_spacing"] - 1.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
