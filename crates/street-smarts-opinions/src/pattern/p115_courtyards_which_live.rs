//! P115 Courtyards Which Live — a courtyard needs real doors opening onto
//! it and a view out to something larger, or it's just a fenced-off gravel
//! pit nobody uses.
//!
//! From Alexander, *A Pattern Language*, Pattern 115, via
//! patternlanguage.cc/Patterns/Courtyards-Which-Live-(115):
//! > **Problem:** The courtyards built in modern buildings are very often
//! > dead... intended to be private open spaces for people to use -- but
//! > they end up unused, full of gravel and abstract sculptures.
//! > **Solution:** Place every courtyard in such a way that there is a
//! > view out of it to some larger open space; place it so that at least
//! > two or three doors open from the building into it and so that the
//! > natural paths which connect these doors pass across the courtyard.
//! > And, at one edge, beside a door, make a roofed veranda or a porch...
//!
//! # A real gap this opinion surfaces, not a hypothetical
//!
//! `p221_natural_doors_and_windows` shapes a courtyard building's hole
//! (courtyard-facing) wall by calling `place_wall_openings(&hole_local,
//! true, None, ...)` -- `door_edge: None`. Reading `place_wall_openings`'s
//! own body: a bay only gets a `Door` when `door_edge == Some(i)`; passing
//! `None` means EVERY bay on the courtyard wall gets a window, never a
//! door. Every courtyard P107/P221 currently produce has real windows
//! facing the courtyard, but literally zero doors opening onto it --
//! exactly the opposite of Alexander's "two or three doors" requirement.
//! This isn't a guess; it's what `door_count` (below) will show on any
//! real courtyard building this pipeline generates today.
//!
//! # Two sub-scores
//!
//! - `door_count`: real count of `Opening { kind: Door, on_hole: true }`
//!   on the building, scored `min(1.0, count / 3.0)` against Alexander's
//!   "two or three."
//! - `larger_view`: whether any OTHER open-space feature in the
//!   neighborhood, larger than this courtyard's own hole area, sits within
//!   `view_threshold_m` (default 150m) of the building -- the same honest
//!   proximity-and-size proxy for "view out to something larger"
//!   `p114_hierarchy_of_open_space` uses, not a real sightline check.
//!
//! `value = 0.5 * door_count + 0.5 * larger_view`. Doesn't attempt to check
//! "natural paths... pass across the courtyard" (no path-routing-through-
//! courtyards data exists) or the roofed veranda/porch requirement (no
//! roof/veranda structure exists in this schema at all) -- named honestly
//! in caveats, not silently dropped.

use std::collections::BTreeMap;
use street_smarts_core::components::BuildingTypology;
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::{Neighborhood, OpeningKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P115CourtyardsWhichLive;

const VIEW_THRESHOLD_M: f64 = 150.0;
const TARGET_DOORS: f64 = 3.0;

impl Opinion for P115CourtyardsWhichLive {
    fn name(&self) -> &'static str {
        "p115_courtyards_which_live"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p115".into(),
            display: "Alexander et al., A Pattern Language, Pattern 115 (Courtyards Which Live)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Courtyards-Which-Live-(115)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let courtyards: Vec<_> = n.buildings.iter()
            .filter(|b| BuildingTypology::label_is_courtyard(b.typology.as_deref()))
            .collect();
        if courtyards.is_empty() {
            return OpinionOutput::NoView {
                reason: "No courtyard-typology buildings in this neighborhood -- P107 hasn't \
                         produced any p107_courtyard_v01 buildings yet."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let other_open_space: Vec<(f64, street_smarts_core::geometry::LngLat)> = n.open_space.iter()
            .filter_map(|o| {
                let area = o.polygon.area_m2();
                if area <= 0.0 {
                    return None;
                }
                Some((area, o.polygon.centroid()))
            })
            .collect();

        let mut door_scores: Vec<f64> = Vec::new();
        let mut view_scores: Vec<f64> = Vec::new();
        let mut no_courtyard_doors: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for b in &courtyards {
            let door_count = b.openings.iter()
                .filter(|o| o.kind == OpeningKind::Door && o.on_hole)
                .count();
            let door_score = (door_count as f64 / TARGET_DOORS).min(1.0);
            door_scores.push(door_score);
            details.insert(format!("{}.courtyard_door_count", b.id), door_count.to_string());
            if door_count == 0 {
                no_courtyard_doors.push(b.id.clone());
            }

            let hole_area = b.polygon.parts_view().first()
                .and_then(|p| p.holes.first())
                .map(|hole| street_smarts_core::geometry::Polygon::from_ring(hole.clone()).area_m2())
                .unwrap_or(0.0);
            let centroid = b.polygon.centroid();
            let has_larger_view = other_open_space.iter().any(|(area, oc)| {
                *area > hole_area && haversine_m(&centroid, oc) <= VIEW_THRESHOLD_M
            });
            view_scores.push(if has_larger_view { 1.0 } else { 0.0 });
        }

        let door_mean = door_scores.iter().sum::<f64>() / door_scores.len() as f64;
        let view_mean = view_scores.iter().sum::<f64>() / view_scores.len() as f64;
        let value = 0.5 * door_mean + 0.5 * view_mean;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("door_count".into(), door_mean);
        sub_scores.insert("larger_view".into(), view_mean);

        details.insert("n_courtyard_buildings".into(), courtyards.len().to_string());
        details.insert("n_with_zero_courtyard_doors".into(), no_courtyard_doors.len().to_string());
        details.insert("target_doors".into(), format!("{TARGET_DOORS:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} courtyard building(s); mean door_count score {:.2} (target: {:.0} doors opening \
                 onto the courtyard), mean larger_view score {:.2}. {} building(s) have ZERO doors \
                 facing their own courtyard.",
                courtyards.len(), door_mean, TARGET_DOORS, view_mean, no_courtyard_doors.len()
            ),
            sub_scores,
            details,
            caveats: vec![
                "Doesn't check that the doors' natural paths actually cross the courtyard -- no \
                 path-routing-through-interior-space data exists in this schema.".into(),
                "Doesn't check for a roofed veranda or porch at all -- no roof/veranda structure \
                 exists in this schema; that half of the pattern is entirely unchecked, not scored \
                 as satisfied by default.".into(),
                "larger_view is the same proximity-and-size proxy p114_hierarchy_of_open_space \
                 uses, not a real sightline/visibility check -- a building between the courtyard \
                 and the larger space would still score full credit here.".into(),
            ],
            contributing_features: no_courtyard_doors,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon, PolygonPart};
    use street_smarts_core::nir::{Building, NeighborhoodMeta, Opening, OpenSpace, OpenSpaceKind};

    fn nbhd(buildings: Vec<Building>, open_space: Vec<OpenSpace>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![],
            buildings,
            streets: vec![],
            open_space,
            boundaries: vec![],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                label: "P115 unit fixture".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
            },
            pattern_fields: vec![],
        }
    }

    fn ring_square(cx: f64, cy: f64, half: f64) -> Vec<LngLat> {
        let m = 1.0 / 111_320.0;
        vec![
            LngLat::new((cx - half) * m, (cy - half) * m),
            LngLat::new((cx + half) * m, (cy - half) * m),
            LngLat::new((cx + half) * m, (cy + half) * m),
            LngLat::new((cx - half) * m, (cy + half) * m),
            LngLat::new((cx - half) * m, (cy - half) * m),
        ]
    }

    fn courtyard_building(id: &str, openings: Vec<Opening>) -> Building {
        Building {
            id: id.into(),
            polygon: Polygon::from_parts(vec![PolygonPart {
                outer: ring_square(0.0, 0.0, 20.0),
                holes: vec![ring_square(0.0, 0.0, 5.0)],
            }]),
            height_m: Some(9.0),
            typology: Some("p107_courtyard_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(3),
            openings,
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], }
    }

    fn door(on_hole: bool) -> Opening {
        Opening { kind: OpeningKind::Door, ring_index: 0, on_hole, t: 0.5, width_m: 0.9, sill_height_m: 0.0, head_height_m: 2.1, floor: 0 }
    }

    #[test]
    fn no_courtyard_buildings_is_no_view() {
        let n = nbhd(vec![], vec![]);
        assert!(matches!(P115CourtyardsWhichLive.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn zero_courtyard_doors_scores_zero_door_count_and_is_flagged() {
        // Matches the real generator's own current output: windows on the
        // hole wall, zero doors -- see this module's own doc comment.
        let n = nbhd(vec![courtyard_building("CY1", vec![door(false)])], vec![]);
        let out = P115CourtyardsWhichLive.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, contributing_features, .. } => {
                assert!((sub_scores["door_count"] - 0.0).abs() < 1e-6);
                assert!(contributing_features.contains(&"CY1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn three_courtyard_doors_scores_full_door_count() {
        let n = nbhd(vec![courtyard_building("CY1", vec![door(true), door(true), door(true)])], vec![]);
        let out = P115CourtyardsWhichLive.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, contributing_features, .. } => {
                assert!((sub_scores["door_count"] - 1.0).abs() < 1e-6);
                assert!(!contributing_features.contains(&"CY1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn two_courtyard_doors_scores_two_thirds() {
        let n = nbhd(vec![courtyard_building("CY1", vec![door(true), door(true)])], vec![]);
        let out = P115CourtyardsWhichLive.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["door_count"] - 2.0 / 3.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_larger_nearby_open_space_satisfies_larger_view() {
        let square = OpenSpace {
            id: "PARK1".into(),
            polygon: Polygon::from_ring(ring_square(60.0, 0.0, 40.0)),
            kind: OpenSpaceKind::Park,
        };
        let n = nbhd(vec![courtyard_building("CY1", vec![door(true), door(true), door(true)])], vec![square]);
        let out = P115CourtyardsWhichLive.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, value, .. } => {
                assert!((sub_scores["larger_view"] - 1.0).abs() < 1e-6);
                assert!((value - 1.0).abs() < 1e-6, "full doors + larger view nearby should score 1.0, got {value}");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
