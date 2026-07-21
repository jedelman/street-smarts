//! P133 Staircase as a Stage — did every multi-story building with a real
//! common room actually get a stair carved into it, open to the room it
//! interrupts?
//!
//! From Alexander, *A Pattern Language*, Pattern 133:
//! > Place the staircase so that the stairs, seen from the room, are like
//! > a stage... arriving, leaving, being seen crossing between floors.
//!
//! # What this checks
//! `p133_staircase_as_a_stage` targets every building with `floors >= 2`
//! and at least one `is_common` interior cell, carving a `kind = "stair"`
//! strip out of that cell. This opinion checks that every building
//! matching that same filter -- the operator's own eligibility test,
//! reused here rather than re-derived -- actually ended up with a stair
//! cell. A building that qualified but got skipped (the exact silent-
//! failure class the operator's own module doc describes having found
//! once already, from `floors` not being set yet when it ran too early)
//! shows up here as a real, aggregable gap instead of only a missing log line.

use std::collections::BTreeMap;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P133StaircaseAsAStage;

impl Opinion for P133StaircaseAsAStage {
    fn name(&self) -> &'static str {
        "p133_staircase_as_a_stage"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p133".into(),
            display: "Alexander et al., A Pattern Language, Pattern 133 (Staircase as a Stage)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl133/apl133.htm".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        // Same eligibility filter p133_staircase_as_a_stage itself uses --
        // multi-story with a real common room to carve the stair out of.
        let eligible: Vec<_> = n
            .buildings
            .iter()
            .filter(|b| (b.floors.unwrap_or(1)) >= 2 && b.interior_cells.iter().any(|c| c.is_common))
            .collect();

        if eligible.is_empty() {
            return OpinionOutput::NoView {
                reason: "No multi-story building with a common-area cell exists yet -- run \
                         p221_natural_doors_and_windows (for real floor counts) and \
                         p129_common_areas_at_the_heart (for a common cell) first."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut n_with_stair = 0usize;
        let mut missing: Vec<String> = Vec::new();

        for b in &eligible {
            if b.interior_cells.iter().any(|c| c.kind == "stair") {
                n_with_stair += 1;
            } else {
                missing.push(b.id.clone());
            }
        }

        let value = n_with_stair as f64 / eligible.len() as f64;

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_eligible".into(), eligible.len().to_string());
        details.insert("n_with_stair".into(), n_with_stair.to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{}/{} eligible multi-story building(s) (>=2 floors, has a common cell) have a stair cell carved out.",
                n_with_stair, eligible.len()
            ),
            sub_scores: BTreeMap::new(),
            details,
            caveats: vec![
                "Eligibility uses the operator's own filter (floors >= 2 AND has an is_common \
                 cell) -- a building that SHOULD be eligible but has floors == None because an \
                 upstream operator hasn't run yet won't be counted as a gap here, it's simply \
                 not evaluated. This checks \"did P133 do its job on what it could see,\" not \
                 \"did every building that should eventually get a stair get one.\""
                    .into(),
                "Presence-only: doesn't check the stair strip's real dimensions, its \
                 open-to-the-room visibility, or whether it actually reads as Alexander's \
                 'stage,' only that a kind=\"stair\" cell exists."
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
    use street_smarts_core::nir::{Building, InteriorCell, NeighborhoodMeta};

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
                label: "P133 opinion fixture".into(),
            },
        }
    }

    fn cell(id: &str, kind: &str, is_common: bool) -> InteriorCell {
        InteriorCell {
            id: id.into(),
            polygon: Polygon::from_ring(vec![]),
            depth: 0.5,
            is_common,
            kind: kind.into(),
            connects_to: vec![],
            floor: 0,
        }
    }

    fn building(id: &str, floors: Option<u32>, cells: Vec<InteriorCell>) -> Building {
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
            floors,
            openings: vec![],
            interior_cells: cells,
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], }
    }

    #[test]
    fn no_eligible_buildings_is_no_view() {
        let single_story = building("b0", Some(1), vec![cell("c0", "room", true)]);
        assert!(matches!(P133StaircaseAsAStage.evaluate(&nbhd(vec![single_story])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn eligible_building_without_stair_is_flagged() {
        let b = building("b0", Some(3), vec![cell("c0", "room", true)]);
        match P133StaircaseAsAStage.evaluate(&nbhd(vec![b])) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!(value.abs() < 1e-9, "got {value}");
                assert_eq!(contributing_features, vec!["b0".to_string()]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn eligible_building_with_stair_scores_full() {
        let b = building("b0", Some(3), vec![cell("c0", "room", true), cell("c1", "stair", false)]);
        match P133StaircaseAsAStage.evaluate(&nbhd(vec![b])) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 1.0).abs() < 1e-9, "got {value}");
                assert!(contributing_features.is_empty());
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn ineligible_single_story_building_is_excluded_not_penalized() {
        let single = building("single", Some(1), vec![cell("c0", "room", true)]);
        let multi_missing = building("multi", Some(2), vec![cell("c0", "room", true)]);
        match P133StaircaseAsAStage.evaluate(&nbhd(vec![single, multi_missing])) {
            OpinionOutput::Value { details, contributing_features, .. } => {
                assert_eq!(details["n_eligible"], "1");
                assert_eq!(contributing_features, vec!["multi".to_string()]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
