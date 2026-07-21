//! P162 North Face — the north side of a building should cascade down to
//! the ground so the sun reaches beside it, instead of casting a dead,
//! shadowed strip.
//!
//! From Alexander, *A Pattern Language*, Pattern 162 (p. 761), via
//! patternlanguage.cc/Patterns/North-Face-(162):
//! > **Problem:** Look at the north sides of the buildings which you
//! > know. Almost everywhere you will find that these are the spots
//! > which are dead and dank, gloomy and useless.
//! > **Solution:** Make the north face of the building a cascade which
//! > slopes down to the ground, so that the sun which normally casts a
//! > long shadow to the north strikes the ground immediately beside the
//! > building.
//!
//! # A real, checkable proxy -- now that a real roof exists
//!
//! `p117_sheltering_roof` (`street-smarts-patterns`) is the only real
//! producer of `Building.roof`, and always assigns a real
//! `RoofShape::Shed` (a genuinely asymmetric slope, high on one side and
//! low on the other) with `slope_azimuth_deg = 0.0` -- see its own module
//! doc for why only an asymmetric, specifically-north-low form honestly
//! satisfies this pattern's own specifically-north claim (a symmetric
//! gable's north and south eaves are the same height, and wouldn't single
//! out north the way Alexander's text does).
//!
//! For every building with a real roof, this checks:
//! - `is_shed`: `shape == Shed` -- only an asymmetric form has a real
//!   "north face" distinct from the rest of the roof at all.
//! - `faces_true_north`: `slope_azimuth_deg` within `AZIMUTH_TOLERANCE_DEG`
//!   (15 degrees -- a real, reasoned pipeline tolerance, not an Alexander
//!   figure; his text gives no numeric bearing) of `0.0` (true north,
//!   exact from this pipeline's own real lng/lat, not approximated).
//! - `cascades_low`: `eave_height_m / ridge_height_m` at or below
//!   `MAX_EAVE_TO_RIDGE_RATIO` (0.5 -- another real, reasoned pipeline
//!   proxy, not from Alexander's text) -- the low side is genuinely low
//!   relative to the whole building, not just nominally "the shorter
//!   side."
//!
//! `value` = fraction of roofed buildings where all three hold.
//!
//! Cannot check the real text's most literal claim -- the roof edge
//! actually reaching ground level (0m) so "the sun... strikes the ground
//! immediately beside the building" -- `p117_sheltering_roof`'s own real
//! `eave_height_m` is Alexander's OWN separate P117 figure (a real ~1.9m
//! entrance-eave height, not a ground-touching cascade); `cascades_low`
//! above is an honest ratio proxy for "genuinely low," not a verified
//! ground-contact check.

use std::collections::BTreeMap;
use street_smarts_core::nir::{Neighborhood, RoofShape};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P162NorthFace;

const AZIMUTH_TOLERANCE_DEG: f64 = 15.0;
const MAX_EAVE_TO_RIDGE_RATIO: f64 = 0.5;

fn angular_distance_deg(a: f64, b: f64) -> f64 {
    let diff = (a - b).rem_euclid(360.0);
    diff.min(360.0 - diff)
}

impl Opinion for P162NorthFace {
    fn name(&self) -> &'static str {
        "p162_north_face"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p162".into(),
            display: "Alexander et al., A Pattern Language, Pattern 162 (North Face)".into(),
            url: Some("https://patternlanguage.cc/Patterns/North-Face-(162)".into()),
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
            let is_shed = roof.shape == RoofShape::Shed;
            let faces_true_north = angular_distance_deg(roof.slope_azimuth_deg, 0.0) <= AZIMUTH_TOLERANCE_DEG;
            let ratio = if roof.ridge_height_m > 0.0 { roof.eave_height_m / roof.ridge_height_m } else { f64::INFINITY };
            let cascades_low = ratio <= MAX_EAVE_TO_RIDGE_RATIO;
            let ok = is_shed && faces_true_north && cascades_low;
            details.insert(format!("{}.eave_to_ridge_ratio", b.id), format!("{ratio:.2}"));
            details.insert(format!("{}.slope_azimuth_deg", b.id), format!("{:.1}", roof.slope_azimuth_deg));
            if ok {
                n_ok += 1;
            } else {
                flagged.push(b.id.clone());
            }
        }

        let value = n_ok as f64 / roofed.len() as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("north_cascade_fraction".into(), value);
        details.insert("n_roofed_buildings".into(), roofed.len().to_string());
        details.insert("azimuth_tolerance_deg".into(), format!("{AZIMUTH_TOLERANCE_DEG:.0}"));
        details.insert("max_eave_to_ridge_ratio".into(), format!("{MAX_EAVE_TO_RIDGE_RATIO:.2}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} real roofed building(s) checked; {} ({:.0}%) have a real asymmetric shed roof \
                 sloping to within {AZIMUTH_TOLERANCE_DEG:.0} degrees of true north, low side at or \
                 below {:.0}% of the ridge height.",
                roofed.len(), n_ok, value * 100.0, MAX_EAVE_TO_RIDGE_RATIO * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Doesn't verify the roof's own low edge actually reaches ground level (0m) -- \
                 Alexander's own most literal claim. eave_height_m is p117_sheltering_roof's own \
                 real ~1.9m entrance-eave figure (a separate, real P117 number), and \
                 max_eave_to_ridge_ratio is an honest ratio proxy for 'genuinely low,' not a \
                 verified ground-contact check.".into(),
                "azimuth_tolerance_deg (15) and max_eave_to_ridge_ratio (0.5) are real, reasoned \
                 pipeline proxies -- Alexander's own text gives no numeric bearing or ratio to check \
                 against.".into(),
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
                layer_provenance: Default::default(), label: "P162 unit fixture".into(),
            },
        }
    }

    #[test]
    fn no_roofed_buildings_is_no_view() {
        let n = nbhd(vec![building("B1", None)]);
        assert!(matches!(P162NorthFace.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_shed_roof_facing_true_north_with_a_low_eave_scores_full() {
        let roof = RoofForm { shape: RoofShape::Shed, ridge_height_m: 9.0, eave_height_m: 1.9, slope_azimuth_deg: 0.0 };
        let n = nbhd(vec![building("B1", Some(roof))]);
        match P162NorthFace.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_shed_roof_facing_south_is_flagged() {
        let roof = RoofForm { shape: RoofShape::Shed, ridge_height_m: 9.0, eave_height_m: 1.9, slope_azimuth_deg: 180.0 };
        let n = nbhd(vec![building("SOUTHFACING", Some(roof))]);
        match P162NorthFace.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-9, "got {value}");
                assert_eq!(contributing_features, vec!["SOUTHFACING".to_string()]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_gable_roof_is_flagged_not_asymmetric() {
        let roof = RoofForm { shape: RoofShape::Gable, ridge_height_m: 9.0, eave_height_m: 1.9, slope_azimuth_deg: 0.0 };
        let n = nbhd(vec![building("GABLE", Some(roof))]);
        match P162NorthFace.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 0.0).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn an_eave_too_close_to_ridge_height_is_flagged_not_cascading() {
        // eave/ridge = 8.0/9.0 ~= 0.89, well above the 0.5 ratio threshold.
        let roof = RoofForm { shape: RoofShape::Shed, ridge_height_m: 9.0, eave_height_m: 8.0, slope_azimuth_deg: 0.0 };
        let n = nbhd(vec![building("BARELY_SLOPED", Some(roof))]);
        match P162NorthFace.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 0.0).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn angular_distance_wraps_around_zero() {
        assert!((angular_distance_deg(350.0, 0.0) - 10.0).abs() < 1e-9);
        assert!((angular_distance_deg(10.0, 0.0) - 10.0).abs() < 1e-9);
        assert!((angular_distance_deg(180.0, 0.0) - 180.0).abs() < 1e-9);
    }
}
