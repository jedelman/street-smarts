//! P165 Opening to the Street — a building's public-facing wall should be
//! substantially open (windows, doors), not a blank face broken only by
//! a single narrow entrance.
//!
//! From Alexander, *A Pattern Language*, Pattern 165, via
//! patternlanguage.cc/Patterns/Opening-to-the-Street-(165):
//! > **Problem:** The sight of action is an incentive for action. When
//! > people can see into spaces from the street their world is enlarged
//! > and made richer...
//! > **Solution:** In any public space which depends for its success on
//! > its exposure to the street, open it up, with a fully opening wall
//! > which can be thrown wide open...
//!
//! # An honest, coarse proxy
//!
//! This schema has no "fully opening wall" mechanism, activity-type, or
//! program data -- Alexander's literal solution (a wall that physically
//! swings/slides open, straddling the pedestrian path) can't be checked at
//! all. What IS real: `p221_natural_doors_and_windows` already identifies
//! each building's street-facing wall segment (the edge it places the main
//! Door opening on, via its own `choose_door_wall` heuristic) and places
//! window bays along it. This opinion re-derives that same wall edge from
//! the real `Opening.ring_index` the Door sits on, then checks how much of
//! that edge's real length is covered by opening width (door + windows on
//! the SAME edge) -- a real, checkable physical-transparency proxy for
//! "opens up... exposed to the street," not a claim to verify the pattern's
//! literal opening-mechanism requirement.
//!
//! `value` = fraction of buildings (by count) whose street-wall opening
//! coverage meets `target_coverage` (default 0.4 -- a judgment call, not a
//! number Alexander's text gives; documented as such).

use std::collections::BTreeMap;
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::{Neighborhood, OpeningKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P165OpeningToTheStreet;

const TARGET_COVERAGE: f64 = 0.4;

impl Opinion for P165OpeningToTheStreet {
    fn name(&self) -> &'static str {
        "p165_opening_to_the_street"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p165".into(),
            display: "Alexander et al., A Pattern Language, Pattern 165 (Opening to the Street)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Opening-to-the-Street-(165)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let mut coverages: Vec<f64> = Vec::new();
        let mut below_target: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();
        let mut n_no_door = 0;

        for b in &n.buildings {
            let door = b.openings.iter().find(|o| o.kind == OpeningKind::Door && !o.on_hole);
            let Some(door) = door else {
                n_no_door += 1;
                continue;
            };
            let ring = &b.polygon.outer;
            if door.ring_index + 1 >= ring.len() {
                continue;
            }
            let wall_len = haversine_m(&ring[door.ring_index], &ring[door.ring_index + 1]);
            if wall_len <= 0.0 {
                continue;
            }
            let opening_width: f64 = b.openings.iter()
                .filter(|o| !o.on_hole && o.ring_index == door.ring_index && o.floor == 0)
                .map(|o| o.width_m)
                .sum();
            let coverage = (opening_width / wall_len).min(1.0);
            coverages.push(coverage);
            details.insert(format!("{}.street_wall_coverage", b.id), format!("{coverage:.2}"));
            if coverage < TARGET_COVERAGE {
                below_target.push(b.id.clone());
            }
        }

        if coverages.is_empty() {
            return OpinionOutput::NoView {
                reason: format!(
                    "No building had both a real outer-wall door and a resolvable wall segment to \
                     measure ({} building(s) had no door at all -- run P221 Natural Doors and Windows first).",
                    n_no_door
                ),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let value = coverages.iter().filter(|c| **c >= TARGET_COVERAGE).count() as f64 / coverages.len() as f64;
        let mean_coverage = coverages.iter().sum::<f64>() / coverages.len() as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("meets_target_fraction".into(), value);
        sub_scores.insert("mean_coverage".into(), mean_coverage);

        details.insert("n_buildings_scored".into(), coverages.len().to_string());
        details.insert("n_buildings_no_door".into(), n_no_door.to_string());
        details.insert("target_coverage".into(), format!("{TARGET_COVERAGE:.2}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} building(s) scored; mean ground-floor street-wall opening coverage {:.0}%; \
                 {:.0}% of buildings meet the {:.0}% coverage target.",
                coverages.len(), mean_coverage * 100.0, value * 100.0, TARGET_COVERAGE * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Cannot check Alexander's literal requirement at all -- a wall that physically \
                 opens (folding/sliding), or activity straddling the pedestrian path. This is a \
                 static-transparency proxy (opening width / wall length) only.".into(),
                "target_coverage (0.4) is a judgment call, not a number Alexander's text gives -- \
                 he describes a mechanism, not a percentage.".into(),
                "Ground floor only (Opening.floor == 0) -- doesn't check upper-floor transparency, \
                 which this pattern doesn't distinguish but a real streetscape assessment would.".into(),
                "Assumes the wall edge carrying the main door is the street-facing one, reusing \
                 p221_natural_doors_and_windows's own door-placement heuristic rather than \
                 independently verifying which wall actually faces a real street.".into(),
            ],
            contributing_features: below_target,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{Building, NeighborhoodMeta, Opening};

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
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P165 unit fixture".into(),
            },
        }
    }

    /// A 20m x 10m rectangle -- edge 0 is the 20m-long bottom wall.
    fn rect_building(id: &str, openings: Vec<Opening>) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0),
                LngLat::new(20.0 * m, 0.0),
                LngLat::new(20.0 * m, 10.0 * m),
                LngLat::new(0.0, 10.0 * m),
                LngLat::new(0.0, 0.0),
            ]),
            height_m: Some(9.0), typology: Some("p107_solid_v01".into()),
            year_built: None, parcel_id: None, floors: Some(3), openings,
            interior_cells: vec![],
        }
    }

    fn opening(kind: OpeningKind, ring_index: usize, width_m: f64, floor: u32) -> Opening {
        Opening { kind, ring_index, on_hole: false, t: 0.5, width_m, sill_height_m: 0.0, head_height_m: 2.1, floor }
    }

    #[test]
    fn no_buildings_with_doors_is_no_view() {
        let n = nbhd(vec![rect_building("B1", vec![])]);
        assert!(matches!(P165OpeningToTheStreet.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_lone_narrow_door_scores_low_coverage() {
        // 0.9m door on a 20m wall -> 4.5% coverage, well under 40% target.
        let n = nbhd(vec![rect_building("B1", vec![opening(OpeningKind::Door, 0, 0.9, 0)])]);
        let out = P165OpeningToTheStreet.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, contributing_features, .. } => {
                assert!(sub_scores["mean_coverage"] < 0.1, "got {}", sub_scores["mean_coverage"]);
                assert!(contributing_features.contains(&"B1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn door_plus_wide_windows_meets_the_target() {
        // Door (0.9m) + three 3m windows on the same edge, ground floor = 9.9m / 20m = 49.5%.
        let n = nbhd(vec![rect_building("B1", vec![
            opening(OpeningKind::Door, 0, 0.9, 0),
            opening(OpeningKind::Window, 0, 3.0, 0),
            opening(OpeningKind::Window, 0, 3.0, 0),
            opening(OpeningKind::Window, 0, 3.0, 0),
        ])]);
        let out = P165OpeningToTheStreet.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, value, contributing_features, .. } => {
                assert!(sub_scores["mean_coverage"] > TARGET_COVERAGE, "got {}", sub_scores["mean_coverage"]);
                assert!((value - 1.0).abs() < 1e-6);
                assert!(!contributing_features.contains(&"B1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn upper_floor_windows_on_the_door_edge_dont_count() {
        // Same door as the low-coverage case, plus a floor-1 window on the
        // same edge -- floor filter should exclude it, coverage unchanged.
        let n = nbhd(vec![rect_building("B1", vec![
            opening(OpeningKind::Door, 0, 0.9, 0),
            opening(OpeningKind::Window, 0, 3.0, 1),
        ])]);
        let out = P165OpeningToTheStreet.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!(sub_scores["mean_coverage"] < 0.1, "upper-floor window shouldn't count toward ground-floor coverage, got {}", sub_scores["mean_coverage"]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
