//! P118 Roof Garden — flat, usable sections of roof should be developed
//! as real gardens, reachable directly from a lived-in floor.
//!
//! From Alexander, *A Pattern Language*, Pattern 118 (p. 575), via
//! patternlanguage.cc/Patterns/Roof-Garden-(118):
//! > **Problem:** A vast part of the earth's surface, in a town,
//! > consists of roofs... it is natural, and indeed essential, to make
//! > roofs which take advantage of the sun and air.
//! > **Solution:** Make parts of almost every roof system usable as roof
//! > gardens... always make it possible to walk directly out onto the
//! > roof garden from some lived-in part of the building.
//!
//! # A real check, now that the schema supports it -- no generator yet
//!
//! `RoofForm.occupiable` now exists specifically for this pattern's own
//! core claim ("usable as roof gardens"). `p117_sheltering_roof` still
//! only ever assigns a sloped `RoofShape::Shed` (never `Flat`, never
//! `occupiable: true`) -- so on every real fixture this pipeline ships
//! today, no roof qualifies, and this opinion still returns `NoView`.
//! What changed: the reason is now "no generator produces one yet", not
//! "the schema can't represent this at all" -- and the check below is
//! REAL, not a placeholder, verified against synthetic fixtures in this
//! file's own tests. Checks both the whole-building `roof` field AND any
//! real `roof_segments` (a flat, occupiable segment counts too, e.g. a
//! P116 cascade's own lowest step).
//!
//! Still can't check "always make it possible to walk directly out onto
//! the roof garden from some lived-in part of the building" -- a real
//! access point (stair/hatch from an `InteriorCell` up to the roof) is a
//! separate, still-unbuilt concept `occupiable` doesn't attempt to solve.
//! Treated honestly as a caveat, not silently assumed satisfied.

use std::collections::BTreeMap;
use street_smarts_core::nir::{Building, Neighborhood, RoofShape};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P118RoofGarden;

fn has_occupiable_flat_roof(b: &Building) -> bool {
    let whole = b.roof.as_ref().is_some_and(|r| r.shape == RoofShape::Flat && r.occupiable);
    let segment = b.roof_segments.iter().any(|s| s.form.shape == RoofShape::Flat && s.form.occupiable);
    whole || segment
}

fn has_any_real_roof(b: &Building) -> bool {
    b.roof.is_some() || !b.roof_segments.is_empty()
}

impl Opinion for P118RoofGarden {
    fn name(&self) -> &'static str {
        "p118_roof_garden"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p118".into(),
            display: "Alexander et al., A Pattern Language, Pattern 118 (Roof Garden)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Roof-Garden-(118)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let roofed: Vec<_> = n.buildings.iter().filter(|b| has_any_real_roof(b)).collect();
        if roofed.is_empty() {
            return OpinionOutput::NoView {
                reason: "No building has a real roof yet (RoofForm.occupiable exists in the schema, \
                         but p117_sheltering_roof only ever assigns a sloped Shed roof, never Flat) \
                         -- nothing to check a real roof-garden claim against yet. See this opinion's \
                         own module doc."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut n_gardened = 0usize;
        let mut ungardened: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for b in &roofed {
            let ok = has_occupiable_flat_roof(b);
            details.insert(format!("{}.has_occupiable_flat_roof", b.id), ok.to_string());
            if ok {
                n_gardened += 1;
            } else {
                ungardened.push(b.id.clone());
            }
        }

        let value = n_gardened as f64 / roofed.len() as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("occupiable_flat_roof_fraction".into(), value);
        details.insert("n_roofed_buildings".into(), roofed.len().to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} real roofed building(s) checked; {} ({:.0}%) have at least one real, flat, \
                 occupiable roof plane.",
                roofed.len(), n_gardened, value * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Can't check Alexander's own literal 'walk directly out onto the roof garden from \
                 some lived-in part of the building' -- a real access point (stair/hatch from an \
                 InteriorCell up to the roof) is a separate, still-unbuilt schema concept. \
                 `occupiable: true` alone doesn't confirm real reachability.".into(),
                "No real fixture this pipeline ships today produces an occupiable flat roof -- \
                 p117_sheltering_roof only ever assigns a sloped Shed. This opinion returns NoView \
                 in practice until a real generator exists.".into(),
            ],
            contributing_features: ungardened,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{NeighborhoodMeta, RoofForm, RoofSegment};

    fn m() -> f64 { 111_320.0 }

    fn square_ring(half_side: f64) -> Vec<LngLat> {
        let s = half_side / m();
        vec![
            LngLat::new(-s, -s), LngLat::new(s, -s),
            LngLat::new(s, s), LngLat::new(-s, s), LngLat::new(-s, -s),
        ]
    }

    fn building(id: &str, roof: Option<RoofForm>, roof_segments: Vec<RoofSegment>) -> Building {
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(square_ring(10.0)),
            height_m: Some(9.0), typology: None, year_built: None, parcel_id: None, floors: None,
            openings: vec![], interior_cells: vec![], wall_thickness_m: None, roof,
            roof_segments, canopies: vec![], wall_niches: vec![],
        }
    }

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P118 unit fixture".into(),
            },
        }
    }

    #[test]
    fn no_roofed_buildings_is_no_view() {
        let n = nbhd(vec![building("B1", None, vec![])]);
        assert!(matches!(P118RoofGarden.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_sloped_shed_roof_is_flagged_not_a_garden() {
        let roof = RoofForm { shape: RoofShape::Shed, ridge_height_m: 11.0, eave_height_m: 9.0, slope_azimuth_deg: 0.0, occupiable: false };
        let n = nbhd(vec![building("SHED", Some(roof), vec![])]);
        match P118RoofGarden.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-9, "got {value}");
                assert_eq!(contributing_features, vec!["SHED".to_string()]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_flat_occupiable_whole_building_roof_scores_full() {
        let roof = RoofForm { shape: RoofShape::Flat, ridge_height_m: 9.0, eave_height_m: 9.0, slope_azimuth_deg: 0.0, occupiable: true };
        let n = nbhd(vec![building("GARDEN", Some(roof), vec![])]);
        match P118RoofGarden.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_flat_but_not_occupiable_roof_is_flagged() {
        let roof = RoofForm { shape: RoofShape::Flat, ridge_height_m: 9.0, eave_height_m: 9.0, slope_azimuth_deg: 0.0, occupiable: false };
        let n = nbhd(vec![building("FLATNOTGARDEN", Some(roof), vec![])]);
        match P118RoofGarden.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 0.0).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn an_occupiable_flat_segment_counts_even_without_a_whole_building_roof() {
        let segment = RoofSegment {
            footprint: Polygon::from_ring(square_ring(5.0)),
            form: RoofForm { shape: RoofShape::Flat, ridge_height_m: 9.0, eave_height_m: 9.0, slope_azimuth_deg: 0.0, occupiable: true },
        };
        let n = nbhd(vec![building("SEGGARDEN", None, vec![segment])]);
        match P118RoofGarden.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
