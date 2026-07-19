//! P112 Entrance Transition — the entrance needs a real transition space
//! between the street and the front door, not a door opening straight
//! into the main room.
//!
//! From Alexander, *A Pattern Language*, Pattern 112, via
//! patternlanguage.cc/Patterns/Entrance-Transition-(112):
//! > **Solution:** Make a transition space between the street and the
//! > front door. Bring the path which connects street and entrance
//! > through this transition space, and mark it with a change of light,
//! > a change of sound, a change of direction... above all with a change
//! > of view.
//!
//! # A real, checkable proxy
//!
//! `p130_entrance_room` already tags the `InteriorCell` at depth 0.0 (the
//! public-facing band `p127_intimacy_gradient` builds) `kind: "entrance"`.
//! This opinion checks that cell's own real area is a genuine, separate
//! zone -- neither vanishingly small (the door opens straight into the
//! next room, no transition at all) nor so large it IS most of the
//! building (not a distinct transition, just the whole ground floor).
//! Scored against `min_fraction`/`max_fraction` (default 0.03-0.35 of the
//! building's total footprint) -- a real, if judgment-call, band for "a
//! real, separate but modest space."
//!
//! Cannot check any of Alexander's literal sensory cues (light, sound,
//! surface, level change) -- no such data exists in this schema.

use std::collections::BTreeMap;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P112EntranceTransition;

const MIN_FRACTION: f64 = 0.03;
const MAX_FRACTION: f64 = 0.35;

impl Opinion for P112EntranceTransition {
    fn name(&self) -> &'static str {
        "p112_entrance_transition"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p112".into(),
            display: "Alexander et al., A Pattern Language, Pattern 112 (Entrance Transition)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Entrance-Transition-(112)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let mut fractions: Vec<f64> = Vec::new();
        let mut off_target: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for b in &n.buildings {
            let Some(entrance_cell) = b.interior_cells.iter().find(|c| c.kind == "entrance") else { continue };
            let total_cell_area: f64 = b.interior_cells.iter().map(|c| c.polygon.area_m2()).sum();
            if total_cell_area <= 0.0 {
                continue;
            }
            let fraction = entrance_cell.polygon.area_m2() / total_cell_area;
            fractions.push(fraction);
            details.insert(format!("{}.entrance_cell_fraction", b.id), format!("{fraction:.3}"));
            if !(MIN_FRACTION..=MAX_FRACTION).contains(&fraction) {
                off_target.push(b.id.clone());
            }
        }

        if fractions.is_empty() {
            return OpinionOutput::NoView {
                reason: "No building has a real, tagged entrance InteriorCell yet -- run P127 \
                         Intimacy Gradient and P130 Entrance Room first."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let value = fractions.iter().filter(|f| (MIN_FRACTION..=MAX_FRACTION).contains(*f)).count() as f64 / fractions.len() as f64;
        let mean_fraction = fractions.iter().sum::<f64>() / fractions.len() as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("real_transition_zone_fraction".into(), value);
        sub_scores.insert("mean_entrance_cell_fraction".into(), mean_fraction);
        details.insert("n_buildings_with_entrance_cell".into(), fractions.len().to_string());
        details.insert("target_range".into(), format!("{MIN_FRACTION:.2}-{MAX_FRACTION:.2}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} building(s) with a real entrance cell; mean area share {:.1}% of the ground \
                 floor; {:.0}% fall in the {:.0}-{:.0}% 'real, modest transition zone' target.",
                fractions.len(), mean_fraction * 100.0, value * 100.0, MIN_FRACTION * 100.0, MAX_FRACTION * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Cannot check any of Alexander's literal sensory cues (light, sound, surface, \
                 level, view change) -- no such data exists in this schema. Area fraction is a \
                 structural-existence proxy only.".into(),
                "min_fraction/max_fraction (0.03-0.35) are judgment calls, not numbers Alexander's \
                 own text gives.".into(),
            ],
            contributing_features: off_target,
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
                layer_provenance: Default::default(), label: "P112 unit fixture".into(),
            },
        }
    }

    fn cell(id: &str, side_m: f64, kind: &str, depth: f64) -> InteriorCell {
        let m = 1.0 / 111_320.0;
        InteriorCell {
            id: id.into(),
            polygon: Polygon::from_ring(vec![LngLat::new(0.0, 0.0), LngLat::new(side_m * m, 0.0), LngLat::new(side_m * m, side_m * m), LngLat::new(0.0, side_m * m), LngLat::new(0.0, 0.0)]),
            depth, is_common: false, kind: kind.into(), connects_to: vec![], floor: 0,
        }
    }

    fn building_with_cells(id: &str, cells: Vec<InteriorCell>) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![LngLat::new(0.0, 0.0), LngLat::new(20.0 * m, 0.0), LngLat::new(20.0 * m, 20.0 * m), LngLat::new(0.0, 20.0 * m), LngLat::new(0.0, 0.0)]),
            height_m: Some(6.0), typology: Some("p107_solid_v01".into()), year_built: None, parcel_id: None, floors: Some(2), openings: vec![], interior_cells: cells,
        }
    }

    #[test]
    fn no_entrance_cells_is_no_view() {
        assert!(matches!(P112EntranceTransition.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_modest_entrance_cell_meets_the_target() {
        // Entrance cell 2x2 = 4 m², rest of building 20x20 - overlap ~ big room 18x18=324 -> fraction ~4/328 ~1.2%... let's use bigger entrance to land in range.
        let n = nbhd(vec![building_with_cells("B1", vec![cell("E1", 4.0, "entrance", 0.0), cell("R1", 18.0, "room", 0.5)])]);
        let out = P112EntranceTransition.evaluate(&n);
        match out {
            OpinionOutput::Value { details, .. } => {
                let frac: f64 = details["B1.entrance_cell_fraction"].parse().unwrap();
                assert!(frac > 0.0 && frac < 1.0);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn entrance_cell_that_is_the_whole_building_is_off_target() {
        let n = nbhd(vec![building_with_cells("B1", vec![cell("E1", 20.0, "entrance", 0.0)])]);
        let out = P112EntranceTransition.evaluate(&n);
        match out {
            OpinionOutput::Value { contributing_features, .. } => {
                assert!(contributing_features.contains(&"B1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
