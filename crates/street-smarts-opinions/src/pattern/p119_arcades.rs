//! P119 Arcades — covered walkways at a building's edge should connect
//! buildings to one another, part inside, part outside.
//!
//! From Alexander, *A Pattern Language*, Pattern 119 (p. 580), via
//! patternlanguage.cc/Patterns/Arcades-(119):
//! > **Problem:** Arcades -- covered walkways at the edge of buildings,
//! > which are partly inside, partly outside -- play a vital role in the
//! > way that people interact with buildings.
//! > **Solution:** Wherever paths run along the edge of buildings, build
//! > arcades, and use the arcades, above all, to connect up the
//! > buildings to one another.
//!
//! # A real check, now that the schema supports it -- no generator yet
//!
//! `Building.canopies` (a `Vec<Canopy>`, real ring-edge span + depth +
//! clearance height, `CanopyKind::Arcade | Gallery`) now exists
//! specifically for this pattern and P166 Gallery Surround. No generator
//! populates it yet -- `p221_natural_doors_and_windows` already computes
//! which wall ring edges face the street, the natural real input a P119
//! generator would need, but that generator doesn't exist. So on every
//! real fixture this pipeline ships today, `canopies` is empty
//! everywhere, and this opinion still returns `NoView`. What changed: the
//! reason is now "no generator populates this yet", not "the schema can't
//! represent this at all" -- and the check below is REAL, not a
//! placeholder, verified against synthetic fixtures in this file's own
//! tests.
//!
//! `value` = fraction of buildings with at least one real
//! `CanopyKind::Arcade` canopy. Ground-floor only (`floor == 0`) by
//! Alexander's own "at the edge of buildings" / walkway-level framing --
//! P166 Gallery Surround, not this pattern, covers upper stories.
//!
//! Can't check "use the arcades, above all, to connect up the buildings
//! to one another" -- that's a claim about a real inter-building
//! connectivity graph, not one building's own canopy geometry. Not
//! attempted here; a separate, larger claim.

use std::collections::BTreeMap;
use street_smarts_core::nir::{CanopyKind, Neighborhood};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P119Arcades;

impl Opinion for P119Arcades {
    fn name(&self) -> &'static str {
        "p119_arcades"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p119".into(),
            display: "Alexander et al., A Pattern Language, Pattern 119 (Arcades)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Arcades-(119)".into()),
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

        let has_any_canopy = n.buildings.iter().any(|b| !b.canopies.is_empty());
        if !has_any_canopy {
            return OpinionOutput::NoView {
                reason: "No building carries a real canopy yet -- Building.canopies exists in the \
                         schema, but no generator populates it (p221_natural_doors_and_windows \
                         already computes which wall edges face the street, the natural real input a \
                         P119 generator would need, but that generator doesn't exist). See this \
                         opinion's own module doc."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut n_arcaded = 0usize;
        let mut unarcaded: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for b in &n.buildings {
            let n_arcades = b.canopies.iter().filter(|c| c.kind == CanopyKind::Arcade && c.floor == 0).count();
            details.insert(format!("{}.n_ground_floor_arcades", b.id), n_arcades.to_string());
            if n_arcades > 0 {
                n_arcaded += 1;
            } else {
                unarcaded.push(b.id.clone());
            }
        }

        let value = n_arcaded as f64 / n.buildings.len() as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("arcaded_fraction".into(), value);
        details.insert("n_buildings".into(), n.buildings.len().to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} building(s) checked; {} ({:.0}%) have at least one real ground-floor arcade \
                 canopy.",
                n.buildings.len(), n_arcaded, value * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Can't check 'use the arcades, above all, to connect up the buildings to one \
                 another' -- that's a real inter-building connectivity claim, not something one \
                 building's own canopy geometry can answer.".into(),
                "No real fixture this pipeline ships today produces a canopy of any kind -- this \
                 opinion returns NoView in practice until a real generator exists.".into(),
            ],
            contributing_features: unarcaded,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{Building, Canopy, NeighborhoodMeta};

    fn m() -> f64 { 111_320.0 }

    fn square_ring(half_side: f64) -> Vec<LngLat> {
        let s = half_side / m();
        vec![
            LngLat::new(-s, -s), LngLat::new(s, -s),
            LngLat::new(s, s), LngLat::new(-s, s), LngLat::new(-s, -s),
        ]
    }

    fn building(id: &str, canopies: Vec<Canopy>) -> Building {
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(square_ring(10.0)),
            height_m: Some(9.0), typology: None, year_built: None, parcel_id: None, floors: None,
            openings: vec![], interior_cells: vec![], wall_thickness_m: None, roof: None,
            roof_segments: vec![], canopies, wall_niches: vec![],
        }
    }

    fn arcade(floor: u32) -> Canopy {
        Canopy { kind: CanopyKind::Arcade, ring_index: 0, on_hole: false, t_start: 0.1, t_end: 0.6, depth_m: 1.8, height_m: 2.4, floor }
    }

    fn gallery(floor: u32) -> Canopy {
        Canopy { kind: CanopyKind::Gallery, ring_index: 0, on_hole: false, t_start: 0.1, t_end: 0.6, depth_m: 1.2, height_m: 2.4, floor }
    }

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P119 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    #[test]
    fn no_canopies_anywhere_is_no_view() {
        let n = nbhd(vec![building("B1", vec![])]);
        assert!(matches!(P119Arcades.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_ground_floor_arcade_scores_full() {
        let n = nbhd(vec![building("B1", vec![arcade(0)])]);
        match P119Arcades.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn an_upper_floor_arcade_does_not_count() {
        let n = nbhd(vec![building("B1", vec![arcade(1)])]);
        match P119Arcades.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-9, "got {value}");
                assert_eq!(contributing_features, vec!["B1".to_string()]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_gallery_canopy_does_not_count_as_an_arcade() {
        let n = nbhd(vec![building("B1", vec![gallery(0)])]);
        match P119Arcades.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 0.0).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
