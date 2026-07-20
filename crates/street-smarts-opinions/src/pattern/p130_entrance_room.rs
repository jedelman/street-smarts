//! P130 Entrance Room — does every multi-cell building actually have a
//! labeled entrance cell, and does it sit at the shallowest point in the
//! gradient (depth 0.0) rather than buried somewhere else?
//!
//! From Alexander, *A Pattern Language*, Pattern 130:
//! > Give the entrance room a name... Make it a real transition, not just
//! > a hole in the wall.
//!
//! # What this checks
//! `p130_entrance_room` labels the cell `p127_intimacy_gradient` built at
//! `depth == 0.0` with `kind = "entrance"`. This opinion checks that every
//! building with more than one interior cell has exactly one cell tagged
//! `"entrance"`, and that the tagged cell really is the shallowest one --
//! a building where the label landed on the wrong cell (a bug, not a
//! missing feature) would still show `has_entrance = true` in a naive
//! presence check but fail here.

use std::collections::BTreeMap;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P130EntranceRoom;

impl Opinion for P130EntranceRoom {
    fn name(&self) -> &'static str {
        "p130_entrance_room"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p130".into(),
            display: "Alexander et al., A Pattern Language, Pattern 130 (Entrance Room)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl130/apl130.htm".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let candidates: Vec<_> = n.buildings.iter().filter(|b| b.interior_cells.len() > 1).collect();
        if candidates.is_empty() {
            return OpinionOutput::NoView {
                reason: "No building has more than one interior cell yet -- run \
                         p127_intimacy_gradient first."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let has_any_entrance_tag =
            candidates.iter().any(|b| b.interior_cells.iter().any(|c| c.kind == "entrance"));
        if !has_any_entrance_tag {
            return OpinionOutput::NoView {
                reason: "Buildings have multi-cell gradients but no cell is tagged \"entrance\" \
                         -- run p130_entrance_room first."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut n_correct = 0usize;
        let mut problems: Vec<String> = Vec::new();

        for b in &candidates {
            let entrance_cells: Vec<_> = b.interior_cells.iter().filter(|c| c.kind == "entrance").collect();
            let shallowest_depth = b
                .interior_cells
                .iter()
                .map(|c| c.depth)
                .fold(f64::INFINITY, f64::min);

            let correct = entrance_cells.len() == 1
                && (entrance_cells[0].depth - shallowest_depth).abs() < 1e-9;

            if correct {
                n_correct += 1;
            } else {
                problems.push(b.id.clone());
            }
        }

        let value = n_correct as f64 / candidates.len() as f64;

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_buildings_evaluated".into(), candidates.len().to_string());
        details.insert("n_correct".into(), n_correct.to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{}/{} multi-cell building(s) have exactly one entrance cell, correctly placed at the shallowest depth.",
                n_correct, candidates.len()
            ),
            sub_scores: BTreeMap::new(),
            details,
            caveats: vec![
                "Structural only: checks the label exists, is unique, and sits at the shallowest \
                 depth -- doesn't check the entrance cell's real-world size or whether it reads \
                 as an actual transition space rather than a bare doorway."
                    .into(),
            ],
            contributing_features: problems,
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
                label: "P130 opinion fixture".into(),
            },
        }
    }

    fn cell(id: &str, depth: f64, kind: &str) -> InteriorCell {
        InteriorCell {
            id: id.into(),
            polygon: Polygon::from_ring(vec![]),
            depth,
            is_common: false,
            kind: kind.into(),
            connects_to: vec![],
            floor: 0,
        }
    }

    fn building(id: &str, cells: Vec<InteriorCell>) -> Building {
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
            floors: Some(1),
            openings: vec![],
            interior_cells: cells,
            wall_thickness_m: None,
        }
    }

    #[test]
    fn no_multicell_buildings_is_no_view() {
        assert!(matches!(P130EntranceRoom.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn no_entrance_tags_anywhere_is_no_view() {
        let b = building("b0", vec![cell("c0", 0.0, "room"), cell("c1", 1.0, "room")]);
        assert!(matches!(P130EntranceRoom.evaluate(&nbhd(vec![b])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn correctly_tagged_shallowest_cell_scores_full() {
        let b = building("b0", vec![cell("c0", 0.0, "entrance"), cell("c1", 1.0, "room")]);
        match P130EntranceRoom.evaluate(&nbhd(vec![b])) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 1.0).abs() < 1e-9, "got {value}");
                assert!(contributing_features.is_empty());
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn entrance_tag_on_wrong_cell_is_flagged() {
        // "entrance" tag landed on the DEEPEST cell, not the shallowest --
        // a bug this opinion should catch even though a tag exists.
        let b = building("b0", vec![cell("c0", 0.0, "room"), cell("c1", 1.0, "entrance")]);
        match P130EntranceRoom.evaluate(&nbhd(vec![b])) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!(value.abs() < 1e-9, "got {value}");
                assert_eq!(contributing_features, vec!["b0".to_string()]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_entrance_tags_are_flagged() {
        let b = building(
            "b0",
            vec![cell("c0", 0.0, "entrance"), cell("c1", 0.5, "entrance"), cell("c2", 1.0, "room")],
        );
        match P130EntranceRoom.evaluate(&nbhd(vec![b])) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!(value.abs() < 1e-9, "got {value}");
                assert_eq!(contributing_features, vec!["b0".to_string()]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
