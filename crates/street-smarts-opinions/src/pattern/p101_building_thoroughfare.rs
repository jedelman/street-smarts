//! P101 Building Thoroughfare — when circulation is forced indoors, it
//! should function as a real shortcut with open connections at both ends,
//! not a dead-end corridor.
//!
//! From Alexander, *A Pattern Language*, Pattern 101, via
//! patternlanguage.cc/Patterns/Building-Thoroughfare-(101):
//! > **Problem:** When a public building complex cannot be completely
//! > served by outdoor pedestrian streets, a new form of indoor street,
//! > quite different from the conventional corridor, is needed.
//! > **Solution:** Wherever density or climate force the main lines of
//! > circulation indoors, build them as building thoroughfares. Place
//! > each thoroughfare in a position where it functions as a shortcut, as
//! > continuous as possible with the public street outside, with wide
//! > open entrances.
//!
//! # A real, checkable proxy
//!
//! `p131_the_flow_through_rooms` already tags a loop-closing
//! `InteriorCell` `kind: "passage"` (folding in Alexander's own Pattern
//! 132 Short Passages rule directly -- see p131's own module doc for why
//! there's no separate P132 operator). This opinion checks that a real
//! passage cell connects to TWO OR MORE other cells (`connects_to.len()
//! >= 2`) -- a real through-route with openings at both ends, not a
//! single-connection dead-end stub.
//!
//! Cannot check the width (11ft min, 15-20ft typical), ceiling height
//! (15ft min), glazed roof, or "as continuous as possible with the public
//! street outside" -- no corridor-width, ceiling-height, or roof data
//! exists in this schema, and this opinion can't confirm which of the
//! connected cells (if any) is genuinely the public-realm end.

use std::collections::BTreeMap;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P101BuildingThoroughfare;

const MIN_CONNECTIONS: usize = 2;

impl Opinion for P101BuildingThoroughfare {
    fn name(&self) -> &'static str {
        "p101_building_thoroughfare"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p101".into(),
            display: "Alexander et al., A Pattern Language, Pattern 101 (Building Thoroughfare)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Building-Thoroughfare-(101)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let mut real_shortcut: Vec<bool> = Vec::new();
        let mut dead_ends: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for b in &n.buildings {
            for c in b.interior_cells.iter().filter(|c| c.kind == "passage") {
                let n_connections = c.connects_to.len();
                let ok = n_connections >= MIN_CONNECTIONS;
                real_shortcut.push(ok);
                let key = format!("{}.{}", b.id, c.id);
                details.insert(format!("{key}.n_connections"), n_connections.to_string());
                if !ok {
                    dead_ends.push(key);
                }
            }
        }

        if real_shortcut.is_empty() {
            return OpinionOutput::NoView {
                reason: "No 'passage'-kind InteriorCell yet -- run P131 The Flow Through Rooms first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let value = real_shortcut.iter().filter(|b| **b).count() as f64 / real_shortcut.len() as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("real_shortcut_fraction".into(), value);
        details.insert("n_passage_cells".into(), real_shortcut.len().to_string());
        details.insert("min_connections".into(), MIN_CONNECTIONS.to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} passage cell(s); {:.0}% connect to {} or more other cells -- a real \
                 through-route, not a dead-end stub.",
                real_shortcut.len(), value * 100.0, MIN_CONNECTIONS
            ),
            sub_scores,
            details,
            caveats: vec![
                "Cannot check the width (11ft min, 15-20ft typical), ceiling height (15ft min), \
                 or glazed-roof specifications -- no corridor-width, ceiling-height, or roof data \
                 exists in this schema.".into(),
                "Cannot confirm 'as continuous as possible with the public street outside' -- this \
                 opinion checks real internal connectivity only, not whether either end genuinely \
                 opens to the public realm.".into(),
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
    use street_smarts_core::nir::{Building, InteriorCell, NeighborhoodMeta};

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P101 unit fixture".into(),
            },
        }
    }

    fn cell(id: &str, kind: &str, connects_to: Vec<&str>) -> InteriorCell {
        let m = 1.0 / 111_320.0;
        InteriorCell {
            id: id.into(),
            polygon: Polygon::from_ring(vec![LngLat::new(0.0, 0.0), LngLat::new(2.0 * m, 0.0), LngLat::new(2.0 * m, 2.0 * m), LngLat::new(0.0, 2.0 * m), LngLat::new(0.0, 0.0)]),
            depth: 0.5, is_common: false, kind: kind.into(), connects_to: connects_to.into_iter().map(String::from).collect(), floor: 0,
        }
    }

    fn building_with_cells(id: &str, cells: Vec<InteriorCell>) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![LngLat::new(0.0, 0.0), LngLat::new(20.0 * m, 0.0), LngLat::new(20.0 * m, 20.0 * m), LngLat::new(0.0, 20.0 * m), LngLat::new(0.0, 0.0)]),
            height_m: Some(6.0), typology: Some("p107_solid_v01".into()), year_built: None, parcel_id: None, floors: Some(2), openings: vec![], interior_cells: cells,
            wall_thickness_m: None,
        }
    }

    #[test]
    fn no_passage_cells_is_no_view() {
        assert!(matches!(P101BuildingThoroughfare.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_passage_connected_at_both_ends_scores_full() {
        let n = nbhd(vec![building_with_cells("B1", vec![cell("P1", "passage", vec!["R1", "R2"])])]);
        let out = P101BuildingThoroughfare.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_dead_end_passage_is_flagged() {
        let n = nbhd(vec![building_with_cells("B1", vec![cell("P1", "passage", vec!["R1"])])]);
        let out = P101BuildingThoroughfare.evaluate(&n);
        match out {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-6);
                assert!(contributing_features.contains(&"B1.P1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
