//! P139 Farmhouse Kitchen — the kitchen should be a real, generously
//! sized room combined with family space, not an isolated galley.
//!
//! From Alexander, *A Pattern Language*, Pattern 139 (p. 660), via
//! patternlanguage.cc/Patterns/Farmhouse-Kitchen-(139):
//! > **Problem:** The isolated kitchen, separate from the family and
//! > considered as an efficient but unpleasant factory of food is a
//! > hangover from the days of servants.
//! > **Solution:** Make the kitchen bigger than usual, big enough to
//! > include the "family room" space... near the center of the commons...
//! > large enough to hold a good big table and chairs.
//!
//! # A real, checkable proxy -- and a real, honest limit
//!
//! This pipeline's `InteriorCell.kind` only ever takes real values
//! `"room"`, `"passage"`, or `"entrance"` (set by `p127`/`p131`/`p130`)
//! -- there is no "kitchen" kind anywhere, and no furniture/program data
//! at all. The one real, checkable stand-in: `p129_common_areas_at_the_heart`
//! already identifies ONE cell per building as `is_common: true` -- "the
//! heart" of the building, structurally the closest real analog to
//! "family room space." This checks whether that common cell's real
//! area clears `MIN_KITCHEN_AREA_M2` (default 20m² -- generously sized
//! for "a good big table and chairs... counters and stove and sink
//! around the edge," not a token nook).
//!
//! `value` = fraction of buildings whose common cell clears the area
//! threshold. Cannot check anything about it actually BEING a kitchen --
//! no furniture, fixture, or room-function data exists in this schema at
//! all.

use std::collections::BTreeMap;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P139FarmhouseKitchen;

const MIN_KITCHEN_AREA_M2: f64 = 20.0;

impl Opinion for P139FarmhouseKitchen {
    fn name(&self) -> &'static str {
        "p139_farmhouse_kitchen"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p139".into(),
            display: "Alexander et al., A Pattern Language, Pattern 139 (Farmhouse Kitchen)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Farmhouse-Kitchen-(139)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let common_cells: Vec<_> = n.buildings.iter()
            .flat_map(|b| b.interior_cells.iter())
            .filter(|c| c.is_common)
            .collect();
        if common_cells.is_empty() {
            return OpinionOutput::NoView {
                reason: "No building has a real common-area cell yet -- run P129 Common Areas at the Heart first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let areas: Vec<f64> = common_cells.iter().map(|c| c.polygon.area_m2()).collect();
        let n_big_enough = areas.iter().filter(|&&a| a >= MIN_KITCHEN_AREA_M2).count();
        let value = n_big_enough as f64 / areas.len() as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("common_cell_big_enough_fraction".into(), value);

        let mean_area = areas.iter().sum::<f64>() / areas.len() as f64;
        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_common_cells".into(), common_cells.len().to_string());
        details.insert("n_big_enough".into(), n_big_enough.to_string());
        details.insert("mean_common_cell_area_m2".into(), format!("{mean_area:.1}"));
        details.insert("min_kitchen_area_m2".into(), format!("{MIN_KITCHEN_AREA_M2:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} common-area cell(s); {}/{} ({:.0}%) clear the {MIN_KITCHEN_AREA_M2:.0}m² \
                 farmhouse-kitchen size target -- mean common-cell area {mean_area:.1}m².",
                common_cells.len(), n_big_enough, areas.len(), value * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "This is a real area check on the building's own common-area cell -- there is no \
                 'kitchen' concept anywhere in this pipeline's schema, so this cannot confirm the \
                 space actually IS a kitchen, only that the structurally closest real analog (the \
                 common cell p129 already identifies) is generously sized.".into(),
                "Cannot check furniture ('a good big table and chairs') or fixtures ('counters and \
                 stove and sink') -- no furniture/fixture data exists in this schema at all.".into(),
                "Cannot check 'near the center of the commons' specifically (a site-scale claim) -- \
                 only the cell's own area, at building scale.".into(),
            ],
            contributing_features: vec![],
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
                layer_provenance: Default::default(), label: "P139 unit fixture".into(),
            },
        }
    }

    fn square_ring(side: f64) -> Vec<LngLat> {
        let m = 1.0 / 111_320.0;
        vec![
            LngLat::new(0.0, 0.0), LngLat::new(side * m, 0.0),
            LngLat::new(side * m, side * m), LngLat::new(0.0, side * m),
        ]
    }

    fn building_with_common_cell(id: &str, side: f64) -> Building {
        let cell = InteriorCell {
            id: format!("{id}_cell"), polygon: Polygon::from_ring(square_ring(side)),
            depth: 0.5, is_common: true, kind: "room".into(), connects_to: vec![], floor: 0,
        };
        Building {
            id: id.into(), polygon: Polygon::from_ring(square_ring(side * 2.0)),
            height_m: Some(9.0), typology: None, year_built: None, parcel_id: None,
            floors: None, openings: vec![], interior_cells: vec![cell],
            wall_thickness_m: None,
            roof: None,
        }
    }

    #[test]
    fn no_common_cells_is_no_view() {
        assert!(matches!(P139FarmhouseKitchen.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_generously_sized_common_cell_clears_the_threshold() {
        // 6m side -> 36 m^2, well over the 20 m^2 target.
        let n = nbhd(vec![building_with_common_cell("B1", 6.0)]);
        let out = P139FarmhouseKitchen.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["common_cell_big_enough_fraction"] - 1.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_token_common_cell_fails_the_threshold() {
        // 2m side -> 4 m^2, well under the 20 m^2 target.
        let n = nbhd(vec![building_with_common_cell("B1", 2.0)]);
        let out = P139FarmhouseKitchen.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["common_cell_big_enough_fraction"] - 0.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
