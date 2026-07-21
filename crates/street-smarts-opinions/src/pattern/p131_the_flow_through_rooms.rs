//! P131 The Flow Through Rooms — did the generated connectivity actually
//! achieve a loop, or did it settle for a plain, dead-ending chain?
//!
//! From Alexander, *A Pattern Language*, Pattern 131:
//! > ...place the common rooms to form a chain, or loop, so that it
//! > becomes possible to walk from room to room... Even better, is the
//! > case where there is a loop.
//!
//! # Why this is a real check, not a tautology
//! `p131_the_flow_through_rooms` itself already reports, in its own trace,
//! how many buildings it closed into a loop vs. left as a chain -- but
//! that's log text, not a first-class, aggregable chorus value the
//! conflict engine or a coalition can weigh against every other opinion.
//! This opinion turns that same distinction into a real `Value`, and adds
//! the one thing the operator's own trace doesn't: a single, typology-
//! agnostic structural test (every cell has connectivity degree >= 2 --
//! no dead ends anywhere in the building) that works identically whether
//! the loop came for free (a courtyard ring, closed by construction) or
//! was earned (a solid building's chain closed by an added passage cell).
//! A courtyard building's ring is trivially always a full loop; the
//! interesting, contested signal here is the solid-building sub-score,
//! which reports how often the generator's own length/width thresholds
//! (`PASSAGE_MAX_LENGTH_M`, `min_width_for_passage_m`) actually let a
//! solid building earn the "even better" loop Alexander names, rather
//! than settling for the chain his text says is legitimate but weaker.
//!
//! # What this checks
//! For every building with more than one interior cell and connectivity
//! data (`p131_the_flow_through_rooms` has run), a building "achieves a
//! loop" when every one of its cells has `connects_to.len() >= 2` -- no
//! dead ends anywhere, regardless of typology or how the loop was closed.
//! `value` is the building-count-weighted fraction that qualify, with
//! `courtyard_fraction` / `solid_fraction` sub-scores splitting out the
//! free case from the earned one.

use std::collections::BTreeMap;
use street_smarts_core::components::BuildingTypology;
use street_smarts_core::nir::{Building, Neighborhood};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P131TheFlowThroughRooms;

fn achieves_loop(b: &Building) -> bool {
    !b.interior_cells.is_empty() && b.interior_cells.iter().all(|c| c.connects_to.len() >= 2)
}

impl Opinion for P131TheFlowThroughRooms {
    fn name(&self) -> &'static str {
        "p131_the_flow_through_rooms"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p131".into(),
            display: "Alexander et al., A Pattern Language, Pattern 131 (The Flow Through Rooms)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl131/apl131.htm".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let candidates: Vec<&Building> = n
            .buildings
            .iter()
            .filter(|b| b.interior_cells.len() > 1)
            .collect();

        if candidates.is_empty() {
            return OpinionOutput::NoView {
                reason: "No building has more than one interior cell yet -- run \
                         p127_intimacy_gradient first."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let has_any_connectivity = candidates
            .iter()
            .any(|b| b.interior_cells.iter().any(|c| !c.connects_to.is_empty()));
        if !has_any_connectivity {
            return OpinionOutput::NoView {
                reason: "Buildings have multi-cell gradients but connects_to is empty everywhere \
                         -- run p131_the_flow_through_rooms first."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut n_loop = 0;
        let mut n_courtyard = 0;
        let mut n_courtyard_loop = 0;
        let mut n_solid = 0;
        let mut n_solid_loop = 0;
        let mut open_chains: Vec<String> = Vec::new();

        for b in &candidates {
            let is_courtyard = BuildingTypology::label_is_courtyard(b.typology.as_deref());
            let looped = achieves_loop(b);
            if looped {
                n_loop += 1;
            } else {
                open_chains.push(b.id.clone());
            }
            if is_courtyard {
                n_courtyard += 1;
                if looped {
                    n_courtyard_loop += 1;
                }
            } else {
                n_solid += 1;
                if looped {
                    n_solid_loop += 1;
                }
            }
        }

        let value = n_loop as f64 / candidates.len() as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        if n_courtyard > 0 {
            sub_scores.insert("courtyard_fraction".into(), n_courtyard_loop as f64 / n_courtyard as f64);
        }
        if n_solid > 0 {
            sub_scores.insert("solid_fraction".into(), n_solid_loop as f64 / n_solid as f64);
        }

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_buildings_evaluated".into(), candidates.len().to_string());
        details.insert("n_loop".into(), n_loop.to_string());
        details.insert("n_open_chain".into(), open_chains.len().to_string());
        details.insert("n_courtyard".into(), n_courtyard.to_string());
        details.insert("n_solid".into(), n_solid.to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} of {} building(s) achieve a real loop (no dead-end cells): {} courtyard \
                 (free by ring shape), {} solid (earned by an added passage).",
                n_loop,
                candidates.len(),
                n_courtyard_loop,
                n_solid_loop
            ),
            sub_scores,
            details,
            caveats: vec![
                "\"Achieves a loop\" is a pure graph test (every cell has connects_to.len() >= 2) \
                 -- it doesn't see whether the physical passage is comfortable to walk, only that \
                 one exists.".into(),
                "courtyard_fraction is close to a tautology: a courtyard building's ring is closed \
                 by construction in p131_the_flow_through_rooms, so it will read near 1.0 unless \
                 something upstream produced a genuinely broken ring. solid_fraction is the \
                 contested signal -- it reports how often the generator's own \
                 PASSAGE_MAX_LENGTH_M / min_width_for_passage_m thresholds actually let a solid \
                 building earn a loop rather than settle for a chain.".into(),
                "Alexander's own text calls a plain chain (no loop) legitimate on its own, not a \
                 failure -- this opinion still scores it below a loop, since his text is explicit \
                 that a loop is 'even better.' A low value here is a real, honest gap, not \
                 necessarily a bug.".into(),
            ],
            contributing_features: open_chains,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{InteriorCell, NeighborhoodMeta};

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
                label: "P131 opinion fixture".into(),
            },
        }
    }

    fn cell(id: &str, connects_to: Vec<&str>) -> InteriorCell {
        InteriorCell {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0),
                LngLat::new(0.0001, 0.0),
                LngLat::new(0.0001, 0.0001),
                LngLat::new(0.0, 0.0001),
                LngLat::new(0.0, 0.0),
            ]),
            depth: 0.5,
            is_common: false,
            kind: "room".into(),
            connects_to: connects_to.into_iter().map(String::from).collect(),
            floor: 0,
        }
    }

    fn building(id: &str, typology: &str, cells: Vec<InteriorCell>) -> Building {
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
            typology: Some(typology.into()),
            year_built: None,
            parcel_id: None,
            floors: Some(1),
            openings: vec![],
            interior_cells: cells,
            wall_thickness_m: None,
            roof: None,
        }
    }

    #[test]
    fn no_data_is_no_view() {
        let n = nbhd(vec![]);
        assert!(matches!(P131TheFlowThroughRooms.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn ungraphed_multicell_building_is_no_view() {
        let b = building(
            "B1",
            "p107_solid_v01",
            vec![cell("c0", vec![]), cell("c1", vec![]), cell("c2", vec![])],
        );
        let n = nbhd(vec![b]);
        assert!(matches!(P131TheFlowThroughRooms.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn open_chain_does_not_qualify() {
        let b = building(
            "B1",
            "p107_solid_v01",
            vec![
                cell("c0", vec!["c1"]),
                cell("c1", vec!["c0", "c2"]),
                cell("c2", vec!["c1"]),
            ],
        );
        let n = nbhd(vec![b]);
        match P131TheFlowThroughRooms.evaluate(&n) {
            OpinionOutput::Value { value, sub_scores, contributing_features, .. } => {
                assert!(value < 1e-9, "got {value}");
                assert!((sub_scores["solid_fraction"]).abs() < 1e-9);
                assert!(contributing_features.contains(&"B1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn solid_chain_closed_with_a_passage_qualifies() {
        let b = building(
            "B1",
            "p107_solid_v01",
            vec![
                cell("c0", vec!["c1", "passage"]),
                cell("c1", vec!["c0", "c2"]),
                cell("c2", vec!["c1", "passage"]),
                cell("passage", vec!["c0", "c2"]),
            ],
        );
        let n = nbhd(vec![b]);
        match P131TheFlowThroughRooms.evaluate(&n) {
            OpinionOutput::Value { value, sub_scores, .. } => {
                assert!((value - 1.0).abs() < 1e-9, "got {value}");
                assert!((sub_scores["solid_fraction"] - 1.0).abs() < 1e-9);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn courtyard_ring_always_qualifies() {
        let b = building(
            "CY1",
            "p107_courtyard_v01",
            vec![
                cell("b0", vec!["b3", "b1"]),
                cell("b1", vec!["b0", "b2"]),
                cell("b2", vec!["b1", "b3"]),
                cell("b3", vec!["b2", "b0"]),
            ],
        );
        let n = nbhd(vec![b]);
        match P131TheFlowThroughRooms.evaluate(&n) {
            OpinionOutput::Value { value, sub_scores, .. } => {
                assert!((value - 1.0).abs() < 1e-9, "got {value}");
                assert!((sub_scores["courtyard_fraction"] - 1.0).abs() < 1e-9);
                assert!(!sub_scores.contains_key("solid_fraction"));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn mixed_buildings_produce_a_fraction() {
        let looped = building(
            "LOOP",
            "p107_courtyard_v01",
            vec![
                cell("b0", vec!["b1", "b2"]),
                cell("b1", vec!["b0", "b2"]),
                cell("b2", vec!["b1", "b0"]),
            ],
        );
        let chain = building(
            "CHAIN",
            "p107_solid_v01",
            vec![cell("c0", vec!["c1"]), cell("c1", vec!["c0", "c2"]), cell("c2", vec!["c1"])],
        );
        let n = nbhd(vec![looped, chain]);
        match P131TheFlowThroughRooms.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 0.5).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
