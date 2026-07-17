//! P221 Natural Doors and Windows — did every building actually get real
//! openings placed, and does each building have at least one door?
//!
//! From Alexander, *A Pattern Language*, Pattern 221:
//! > Windows... deep bay windows... In this pattern we deal with the
//! > actual construction of ordinary windows.
//!
//! # What this checks
//! Every building `p107_wings_of_light` (or a courtyard equivalent) shaped
//! should, after `p221_natural_doors_and_windows` runs, have a non-empty
//! `openings` list AND at least one opening of kind `Door` -- a building
//! with windows but no door is exactly the kind of silent-partial-failure
//! the operator's own module doc warns is possible (a wall-segment filter
//! that finds windows but never finds a wall facing a street/open space
//! for the door). `value` is the fraction of buildings satisfying both;
//! `has_any_opening_fraction` / `has_door_fraction` sub-scores split the
//! two failure modes apart.

use std::collections::BTreeMap;
use street_smarts_core::nir::{Neighborhood, OpeningKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P221NaturalDoorsAndWindows;

impl Opinion for P221NaturalDoorsAndWindows {
    fn name(&self) -> &'static str {
        "p221_natural_doors_and_windows"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p221".into(),
            display: "Alexander et al., A Pattern Language, Pattern 221 (Natural Doors and Windows)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl221/apl221.htm".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        if n.buildings.is_empty() {
            return OpinionOutput::NoView {
                reason: "No buildings exist yet -- run p107_wings_of_light first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let n_buildings = n.buildings.len();
        let mut n_with_opening = 0usize;
        let mut n_with_door = 0usize;
        let mut missing: Vec<String> = Vec::new();

        for b in &n.buildings {
            let has_opening = !b.openings.is_empty();
            let has_door = b.openings.iter().any(|o| o.kind == OpeningKind::Door);
            if has_opening {
                n_with_opening += 1;
            }
            if has_door {
                n_with_door += 1;
            }
            if !has_opening || !has_door {
                missing.push(b.id.clone());
            }
        }

        if n_with_opening == 0 {
            return OpinionOutput::NoView {
                reason: "Buildings exist but none carry any openings -- run \
                         p221_natural_doors_and_windows first."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let has_any_opening_fraction = n_with_opening as f64 / n_buildings as f64;
        let has_door_fraction = n_with_door as f64 / n_buildings as f64;
        let value = (has_any_opening_fraction + has_door_fraction) / 2.0;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("has_any_opening_fraction".into(), has_any_opening_fraction);
        sub_scores.insert("has_door_fraction".into(), has_door_fraction);

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_buildings".into(), n_buildings.to_string());
        details.insert("n_with_any_opening".into(), n_with_opening.to_string());
        details.insert("n_with_door".into(), n_with_door.to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{}/{} building(s) have at least one opening; {}/{} have at least one door.",
                n_with_opening, n_buildings, n_with_door, n_buildings
            ),
            sub_scores,
            details,
            caveats: vec![
                "Presence-only: counts that an opening/door exists, not whether window sizing \
                 actually shrinks floor-by-floor (Alexander's own cited detail) or whether \
                 placement reads as natural rather than mechanical."
                    .into(),
            ],
            contributing_features: missing,
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
            bbox_wgs84: [0.0, 0.0, 0.001, 0.001],
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
                label: "P221 opinion fixture".into(),
            },
        }
    }

    fn opening(kind: OpeningKind) -> Opening {
        Opening {
            kind,
            ring_index: 0,
            on_hole: false,
            t: 0.5,
            width_m: 1.0,
            sill_height_m: 0.9,
            head_height_m: 2.1,
            floor: 0,
        }
    }

    fn building(id: &str, openings: Vec<Opening>) -> Building {
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0),
                LngLat::new(0.001, 0.0),
                LngLat::new(0.001, 0.001),
                LngLat::new(0.0, 0.001),
                LngLat::new(0.0, 0.0),
            ]),
            height_m: Some(7.0),
            typology: None,
            year_built: None,
            parcel_id: None,
            floors: Some(2),
            openings,
            interior_cells: vec![],
        }
    }

    #[test]
    fn no_buildings_is_no_view() {
        assert!(matches!(P221NaturalDoorsAndWindows.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn no_openings_anywhere_is_no_view() {
        let n = nbhd(vec![building("b0", vec![])]);
        assert!(matches!(P221NaturalDoorsAndWindows.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn windows_without_a_door_is_flagged() {
        let n = nbhd(vec![building("b0", vec![opening(OpeningKind::Window), opening(OpeningKind::Window)])]);
        match P221NaturalDoorsAndWindows.evaluate(&n) {
            OpinionOutput::Value { sub_scores, contributing_features, .. } => {
                assert!((sub_scores["has_any_opening_fraction"] - 1.0).abs() < 1e-9);
                assert!(sub_scores["has_door_fraction"].abs() < 1e-9);
                assert_eq!(contributing_features, vec!["b0".to_string()]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn building_with_door_and_windows_scores_full() {
        let n = nbhd(vec![building(
            "b0",
            vec![opening(OpeningKind::Door), opening(OpeningKind::Window)],
        )]);
        match P221NaturalDoorsAndWindows.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 1.0).abs() < 1e-9, "got {value}");
                assert!(contributing_features.is_empty());
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn mixed_buildings_produce_a_fraction() {
        let n = nbhd(vec![
            building("good", vec![opening(OpeningKind::Door), opening(OpeningKind::Window)]),
            building("bad", vec![]),
        ]);
        match P221NaturalDoorsAndWindows.evaluate(&n) {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["has_any_opening_fraction"] - 0.5).abs() < 1e-9);
                assert!((sub_scores["has_door_fraction"] - 0.5).abs() < 1e-9);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
