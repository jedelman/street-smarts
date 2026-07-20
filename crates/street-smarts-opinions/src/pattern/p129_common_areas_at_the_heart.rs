//! P129 Common Areas at the Heart — is the cell `p129_common_areas_at_the_heart`
//! marked `is_common` actually ON the path between other cells, or did the
//! nearest-to-centroid pick land on a dead end?
//!
//! From Alexander, *A Pattern Language*, Pattern 129:
//! > Locate [the common area] at the center of gravity of all the spaces
//! > the group occupies, and in such a way that the paths which go in and
//! > out of the building lie tangent to it.
//!
//! # Why this is a real check, not a tautology
//! `p129_common_areas_at_the_heart` itself only ever asks "which cell is
//! nearest the plan's center of gravity" -- it has no idea, at the point it
//! runs, which cells will end up connected to which (`p131_the_flow_through_
//! rooms` fills in `connects_to` afterward). Its own module doc argues the
//! "tangent, not through the middle" half of Alexander's rule follows for
//! free from `p127_intimacy_gradient`'s partition discipline (cells only
//! ever border immediate neighbors) -- true, but that argument never
//! actually establishes that the geometrically-nearest-to-centroid cell
//! sits mid-chain rather than at an end. A short chain, or a lopsided
//! footprint whose centroid sits closer to one end than the true middle,
//! can pick an end cell as "common" -- which Alexander's own criterion
//! (paths lie TANGENT to it, implying it's passed through, not a
//! destination you have to double back from) would call a failure. This
//! opinion is the only place in the pipeline that checks the connectivity
//! graph `p131_the_flow_through_rooms` actually built against that claim.
//!
//! # What this checks
//! For every building with `connects_to` data (so `p131_the_flow_through_
//! rooms` has run) and more than one interior cell, find the cell flagged
//! `is_common` and check its degree in the `connects_to` graph. Degree
//! `>= 2` means at least two different paths pass through it -- exactly
//! what "tangent" requires. Degree `<= 1` means it's a dead end: reachable,
//! but not something you pass through on the way to somewhere else.
//! `value` is the building-count-weighted fraction that qualify.

use std::collections::BTreeMap;
use street_smarts_core::nir::{Building, Neighborhood};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P129CommonAreasAtTheHeart;

impl Opinion for P129CommonAreasAtTheHeart {
    fn name(&self) -> &'static str {
        "p129_common_areas_at_the_heart"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p129".into(),
            display: "Alexander et al., A Pattern Language, Pattern 129 (Common Areas at the Heart)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl129/apl129.htm".into()),
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
            .filter(|b| b.interior_cells.len() > 1 && b.interior_cells.iter().any(|c| c.is_common))
            .collect();

        if candidates.is_empty() {
            return OpinionOutput::NoView {
                reason: "No building has both a common-area flag and more than one interior cell \
                         yet -- run p127_intimacy_gradient then p129_common_areas_at_the_heart."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let has_any_connectivity = candidates
            .iter()
            .any(|b| b.interior_cells.iter().any(|c| !c.connects_to.is_empty()));
        if !has_any_connectivity {
            return OpinionOutput::NoView {
                reason: "Common-area cells are flagged but connects_to is empty everywhere -- run \
                         p131_the_flow_through_rooms first."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut n_on_path = 0;
        let mut dead_ends: Vec<String> = Vec::new();

        for b in &candidates {
            let Some(common) = b.interior_cells.iter().find(|c| c.is_common) else {
                continue;
            };
            if common.connects_to.len() >= 2 {
                n_on_path += 1;
            } else {
                dead_ends.push(b.id.clone());
            }
        }

        let value = n_on_path as f64 / candidates.len() as f64;
        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_buildings_evaluated".into(), candidates.len().to_string());
        details.insert("n_on_path".into(), n_on_path.to_string());
        details.insert("n_dead_end".into(), dead_ends.len().to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} of {} building(s) with a marked common area have it sitting on the actual \
                 connectivity path (degree >= 2), not at a dead end.",
                n_on_path,
                candidates.len()
            ),
            sub_scores: BTreeMap::new(),
            details,
            caveats: vec![
                "\"Tangent\" is operationalized here purely as graph degree (>= 2 connections) in \
                 the InteriorCell connects_to graph -- a real proxy for 'paths pass through it', not \
                 a geometric check of how the paths actually bend around the cell's shape."
                    .into(),
                "A 2-cell building has no middle to be tangent to -- the common-area pick is forced \
                 to an end, and this opinion will correctly count it as a dead end. That's a real \
                 limit of the pattern at that scale, not a bug in the check.".into(),
                "Only one common area per building is modeled (p129_common_areas_at_the_heart's own \
                 documented limit) -- Alexander's text calls for one per social GROUP, which this \
                 pipeline has no way to identify from form alone.".into(),
            ],
            contributing_features: dead_ends,
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
                label: "P129 opinion fixture".into(),
            },
        }
    }

    fn cell(id: &str, is_common: bool, connects_to: Vec<&str>) -> InteriorCell {
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
            is_common,
            kind: "room".into(),
            connects_to: connects_to.into_iter().map(String::from).collect(),
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
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(1),
            openings: vec![],
            interior_cells: cells,
            wall_thickness_m: None,
        }
    }

    #[test]
    fn no_data_is_no_view() {
        let n = nbhd(vec![]);
        assert!(matches!(P129CommonAreasAtTheHeart.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn flagged_but_unconnected_is_no_view() {
        // is_common set, but p131 hasn't run yet -- every connects_to is empty.
        let b = building(
            "B1",
            vec![cell("c0", false, vec![]), cell("c1", true, vec![]), cell("c2", false, vec![])],
        );
        let n = nbhd(vec![b]);
        assert!(matches!(P129CommonAreasAtTheHeart.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn middle_of_chain_qualifies() {
        let b = building(
            "B1",
            vec![
                cell("c0", false, vec!["c1"]),
                cell("c1", true, vec!["c0", "c2"]),
                cell("c2", false, vec!["c1"]),
            ],
        );
        let n = nbhd(vec![b]);
        match P129CommonAreasAtTheHeart.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 1.0).abs() < 1e-9, "got {value}");
                assert!(contributing_features.is_empty());
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn end_of_chain_is_a_dead_end() {
        let b = building(
            "B1",
            vec![
                cell("c0", true, vec!["c1"]),
                cell("c1", false, vec!["c0", "c2"]),
                cell("c2", false, vec!["c1"]),
            ],
        );
        let n = nbhd(vec![b]);
        match P129CommonAreasAtTheHeart.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!(value < 1e-9, "got {value}");
                assert!(contributing_features.contains(&"B1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn courtyard_ring_common_cell_always_qualifies() {
        // Every bay in a closed ring has degree 2 -- whichever one is
        // "common" always sits on the path.
        let b = building(
            "CY1",
            vec![
                cell("b0", false, vec!["b3", "b1"]),
                cell("b1", true, vec!["b0", "b2"]),
                cell("b2", false, vec!["b1", "b3"]),
                cell("b3", false, vec!["b2", "b0"]),
            ],
        );
        let n = nbhd(vec![b]);
        match P129CommonAreasAtTheHeart.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn mixed_buildings_produce_a_fraction() {
        let good = building(
            "GOOD",
            vec![
                cell("g0", false, vec!["g1"]),
                cell("g1", true, vec!["g0", "g2"]),
                cell("g2", false, vec!["g1"]),
            ],
        );
        let bad = building(
            "BAD",
            vec![
                cell("b0", true, vec!["b1"]),
                cell("b1", false, vec!["b0", "b2"]),
                cell("b2", false, vec!["b1"]),
            ],
        );
        let n = nbhd(vec![good, bad]);
        match P129CommonAreasAtTheHeart.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 0.5).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
