//! P110 Main Entrance — a building's entrance should be immediately
//! visible from the main paths of approach, not just present somewhere.
//!
//! From Alexander, *A Pattern Language*, Pattern 110, via
//! patternlanguage.cc/Patterns/Main-Entrance-(110):
//! > **Problem:** Placing the main entrance (or main entrances) is perhaps
//! > the single most important step you take during the evolution of a
//! > building plan.
//! > **Solution:** Place the main entrance of the building at a point
//! > where it can be seen immediately from the main avenues of approach
//! > and give it a bold, visible shape which stands out in front of the
//! > building.
//!
//! # Two real, checkable proxies
//!
//! - `singularity`: Alexander's "the main entrance" (or a small few, not
//!   many) should read as distinct -- scored `1.0 / count` for buildings
//!   with 1+ ground-floor outer doors, `0.0` for buildings with none.
//! - `street_proximity`: straight-line distance from the (one, real)
//!   entrance position to the nearest real `Street` centerline vertex,
//!   scored `1.0` within `visible_threshold_m` (default 40m) falling to 0
//!   by double that -- a proximity proxy for "seen immediately from the
//!   main avenues of approach," not a real sightline/visibility check.
//!
//! `value = 0.5 * singularity + 0.5 * street_proximity`. Doesn't attempt
//! to check "bold, visible shape which stands out" -- this schema has no
//! architectural-massing/facade-articulation data beyond door width, and
//! comparing door width alone isn't a real proxy for visual prominence;
//! named honestly in caveats rather than faked with a weak signal.

use std::collections::BTreeMap;
use street_smarts_core::geometry::{haversine_m, LngLat};
use street_smarts_core::nir::{Neighborhood, OpeningKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P110MainEntrance;

const VISIBLE_THRESHOLD_M: f64 = 40.0;

impl Opinion for P110MainEntrance {
    fn name(&self) -> &'static str {
        "p110_main_entrance"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p110".into(),
            display: "Alexander et al., A Pattern Language, Pattern 110 (Main Entrance)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Main-Entrance-(110)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        if n.buildings.is_empty() {
            return OpinionOutput::NoView {
                reason: "No buildings in this neighborhood.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let street_vertices: Vec<LngLat> = n.streets.iter().flat_map(|s| s.centerline.iter().copied()).collect();

        let mut singularity_scores: Vec<f64> = Vec::new();
        let mut proximity_scores: Vec<f64> = Vec::new();
        let mut no_entrance: Vec<String> = Vec::new();
        let mut far_from_street: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();
        let mut n_scored = 0;

        for b in &n.buildings {
            let doors: Vec<_> = b.openings.iter()
                .filter(|o| o.kind == OpeningKind::Door && !o.on_hole && o.floor == 0)
                .collect();
            if doors.is_empty() {
                singularity_scores.push(0.0);
                no_entrance.push(b.id.clone());
                continue;
            }
            n_scored += 1;
            let singularity = 1.0 / doors.len() as f64;
            singularity_scores.push(singularity);
            details.insert(format!("{}.n_entrances", b.id), doors.len().to_string());

            if street_vertices.is_empty() {
                continue;
            }
            let door = doors[0];
            let ring = &b.polygon.outer;
            if door.ring_index + 1 >= ring.len() {
                continue;
            }
            let a = ring[door.ring_index];
            let c = ring[door.ring_index + 1];
            let pos = LngLat::new(a.lng + (c.lng - a.lng) * door.t, a.lat + (c.lat - a.lat) * door.t);
            let nearest_m = street_vertices.iter().map(|v| haversine_m(&pos, v)).fold(f64::MAX, f64::min);
            let score = (1.0 - (nearest_m - VISIBLE_THRESHOLD_M).max(0.0) / VISIBLE_THRESHOLD_M).clamp(0.0, 1.0);
            proximity_scores.push(score);
            details.insert(format!("{}.nearest_street_m", b.id), format!("{nearest_m:.0}"));
            if score < 0.5 {
                far_from_street.push(b.id.clone());
            }
        }

        if n_scored == 0 {
            return OpinionOutput::NoView {
                reason: "No building has a real ground-floor entrance yet -- run P221 Natural \
                         Doors and Windows first."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let singularity = singularity_scores.iter().sum::<f64>() / singularity_scores.len() as f64;
        let proximity = if proximity_scores.is_empty() {
            None
        } else {
            Some(proximity_scores.iter().sum::<f64>() / proximity_scores.len() as f64)
        };
        let value = match proximity {
            Some(p) => 0.5 * singularity + 0.5 * p,
            None => singularity,
        };

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("singularity".into(), singularity);
        if let Some(p) = proximity {
            sub_scores.insert("street_proximity".into(), p);
        }

        details.insert("n_buildings".into(), n.buildings.len().to_string());
        details.insert("n_no_entrance".into(), no_entrance.len().to_string());

        let mut contributing = no_entrance.clone();
        for id in &far_from_street {
            if !contributing.contains(id) {
                contributing.push(id.clone());
            }
        }

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} building(s); mean entrance singularity {:.2}; {}",
                n.buildings.len(),
                singularity,
                proximity.map(|p| format!("mean street-proximity score {p:.2}.")).unwrap_or_else(|| "no streets to check proximity against.".into()),
            ),
            sub_scores,
            details,
            caveats: vec![
                "Doesn't check 'a bold, visible shape which stands out' at all -- no facade-\
                 articulation or architectural-massing data exists in this schema to check visual \
                 prominence.".into(),
                "street_proximity is straight-line distance to the nearest street CENTERLINE \
                 VERTEX, not a real sightline -- a building whose entrance faces away from a \
                 nearby street still scores well if the street happens to be close.".into(),
                "Buildings with zero entrances score 0.0 on singularity outright (not NoView per \
                 building) -- run P221 Natural Doors and Windows before trusting this opinion's \
                 score is meaningful.".into(),
            ],
            contributing_features: contributing,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{Building, NeighborhoodMeta, Opening, Street};

    fn nbhd(buildings: Vec<Building>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings, streets, open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P110 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn rect_building(id: &str, openings: Vec<Opening>) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0), LngLat::new(10.0 * m, 0.0),
                LngLat::new(10.0 * m, 10.0 * m), LngLat::new(0.0, 10.0 * m), LngLat::new(0.0, 0.0),
            ]),
            height_m: Some(6.0), typology: Some("p107_solid_v01".into()),
            year_built: None, parcel_id: None, floors: Some(2), openings,
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], }
    }

    fn door() -> Opening {
        Opening { kind: OpeningKind::Door, ring_index: 0, on_hole: false, t: 0.5, width_m: 0.9, sill_height_m: 0.0, head_height_m: 2.1, floor: 0 }
    }

    #[test]
    fn no_buildings_is_no_view() {
        assert!(matches!(P110MainEntrance.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn no_entrances_at_all_is_no_view() {
        let n = nbhd(vec![rect_building("B1", vec![])], vec![]);
        assert!(matches!(P110MainEntrance.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn one_entrance_scores_full_singularity() {
        let n = nbhd(vec![rect_building("B1", vec![door()])], vec![]);
        let out = P110MainEntrance.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["singularity"] - 1.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn three_equal_entrances_score_partial_singularity() {
        let n = nbhd(vec![rect_building("B1", vec![door(), door(), door()])], vec![]);
        let out = P110MainEntrance.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["singularity"] - (1.0 / 3.0)).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn entrance_near_a_real_street_scores_high_proximity() {
        let m = 1.0 / 111_320.0;
        let street = Street { id: "S1".into(), centerline: vec![LngLat::new(5.0 * m, -5.0 * m), LngLat::new(5.0 * m, -20.0 * m)], classification: Some("local".into()), row_width_m: Some(8.0), surface: None };
        let n = nbhd(vec![rect_building("B1", vec![door()])], vec![street]);
        let out = P110MainEntrance.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!(sub_scores["street_proximity"] > 0.8, "got {}", sub_scores["street_proximity"]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
