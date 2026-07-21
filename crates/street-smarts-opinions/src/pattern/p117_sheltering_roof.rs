//! P117 Sheltering Roof — the roof should be visible, sloped, and felt
//! as real shelter, with eaves brought down low at entrances.
//!
//! From Alexander, *A Pattern Language*, Pattern 117 (p. 569), via
//! patternlanguage.cc/Patterns/Sheltering-Roof-(117):
//! > **Problem:** The roof plays a primal role in our lives. If the roof
//! > is hidden, if its presence cannot be felt around the building, or
//! > if it cannot be used, then people will lack a fundamental sense of
//! > shelter.
//! > **Solution:** Slope the roof or make a vault of it, make its entire
//! > surface visible, and bring the eaves of the roof down low, as low
//! > as 6'0" or 6'6" at places like the entrance.
//!
//! # A real, checkable proxy -- now that a real roof exists
//!
//! `p117_sheltering_roof` (the generator, `street-smarts-patterns`) is the
//! only real producer of `Building.roof` -- see its own module doc for why
//! it always assigns a real `RoofShape::Shed` (never `Flat`) with a real
//! `eave_height_m`. This opinion checks, for every building with a real
//! roof: is it sloped (any shape but `Flat` -- "entire surface visible"),
//! and does its own `eave_height_m` fall within Alexander's own literal
//! 6'0"-6'6" (1.8288-1.9812m) figure ("eaves down low")? `value` =
//! fraction of roofed buildings where both hold.
//!
//! Cannot check "make its entire surface visible" as an actual unobstructed
//! sightline claim (no viewpoint/occlusion data in this schema) -- treated
//! as satisfied by any real slope, the same proxy category as this
//! project's other geometry-stands-in-for-experience checks. Cannot verify
//! the low eave is specifically AT the entrance either -- see the
//! generator's own module doc for why that's a real, named gap between
//! what P117's text asks and what this operator's single whole-building
//! shed roof actually produces.

use std::collections::BTreeMap;
use street_smarts_core::nir::{Neighborhood, RoofShape};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P117ShelteringRoof;

const MIN_EAVE_M: f64 = 1.8288; // 6'0"
const MAX_EAVE_M: f64 = 1.9812; // 6'6"

impl Opinion for P117ShelteringRoof {
    fn name(&self) -> &'static str {
        "p117_sheltering_roof"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p117".into(),
            display: "Alexander et al., A Pattern Language, Pattern 117 (Sheltering Roof)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Sheltering-Roof-(117)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let roofed: Vec<_> = n.buildings.iter().filter_map(|b| b.roof.as_ref().map(|r| (b, r))).collect();
        if roofed.is_empty() {
            return OpinionOutput::NoView {
                reason: "No building has a real roof yet -- run p117_sheltering_roof (the generator) first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut n_ok = 0usize;
        let mut flagged: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for (b, roof) in &roofed {
            let sloped = roof.shape != RoofShape::Flat;
            let eave_in_range = roof.eave_height_m >= MIN_EAVE_M && roof.eave_height_m <= MAX_EAVE_M;
            let ok = sloped && eave_in_range;
            details.insert(format!("{}.eave_height_m", b.id), format!("{:.2}", roof.eave_height_m));
            if ok {
                n_ok += 1;
            } else {
                flagged.push(b.id.clone());
            }
        }

        let value = n_ok as f64 / roofed.len() as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("sheltering_roof_fraction".into(), value);
        details.insert("n_roofed_buildings".into(), roofed.len().to_string());
        details.insert("min_eave_m".into(), format!("{MIN_EAVE_M:.4}"));
        details.insert("max_eave_m".into(), format!("{MAX_EAVE_M:.4}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} real roofed building(s) checked; {} ({:.0}%) are sloped with a real eave in \
                 Alexander's own literal 6'0\"-6'6\" ({MIN_EAVE_M:.2}-{MAX_EAVE_M:.2}m) range.",
                roofed.len(), n_ok, value * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "\"Make its entire surface visible\" is treated as satisfied by any real slope -- \
                 this schema has no viewpoint/occlusion data to check an actual unobstructed \
                 sightline.".into(),
                "Doesn't verify the low eave sits specifically AT the building's own real entrance \
                 -- see p117_sheltering_roof's own generator module doc for this real, named gap."
                    .into(),
            ],
            contributing_features: flagged,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{Building, NeighborhoodMeta, RoofForm};

    fn m() -> f64 { 111_320.0 }

    fn square_ring(half_side: f64) -> Vec<LngLat> {
        let s = half_side / m();
        vec![
            LngLat::new(-s, -s), LngLat::new(s, -s),
            LngLat::new(s, s), LngLat::new(-s, s), LngLat::new(-s, -s),
        ]
    }

    fn building(id: &str, roof: Option<RoofForm>) -> Building {
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(square_ring(10.0)),
            height_m: Some(9.0), typology: None, year_built: None, parcel_id: None, floors: None,
            openings: vec![], interior_cells: vec![], wall_thickness_m: None,
            roof,
        }
    }

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P117 unit fixture".into(),
            },
        }
    }

    #[test]
    fn no_roofed_buildings_is_no_view() {
        let n = nbhd(vec![building("B1", None)]);
        assert!(matches!(P117ShelteringRoof.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_shed_roof_with_the_real_default_eave_scores_full() {
        let roof = RoofForm { shape: RoofShape::Shed, ridge_height_m: 9.0, eave_height_m: 1.9, slope_azimuth_deg: 0.0 };
        let n = nbhd(vec![building("B1", Some(roof))]);
        match P117ShelteringRoof.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_flat_roof_is_flagged_not_sloped() {
        let roof = RoofForm { shape: RoofShape::Flat, ridge_height_m: 9.0, eave_height_m: 1.9, slope_azimuth_deg: 0.0 };
        let n = nbhd(vec![building("FLAT", Some(roof))]);
        match P117ShelteringRoof.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-9, "got {value}");
                assert_eq!(contributing_features, vec!["FLAT".to_string()]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn an_eave_outside_alexanders_literal_range_is_flagged() {
        let roof = RoofForm { shape: RoofShape::Shed, ridge_height_m: 9.0, eave_height_m: 3.0, slope_azimuth_deg: 0.0 };
        let n = nbhd(vec![building("TOOHIGH", Some(roof))]);
        match P117ShelteringRoof.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 0.0).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
