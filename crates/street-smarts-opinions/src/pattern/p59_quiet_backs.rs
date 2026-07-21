//! P59 Quiet Backs — buildings fronting a busy street need a real quiet
//! side, well away from that street, not noise on every side.
//!
//! From Alexander, *A Pattern Language*, Pattern 59 (p. 301), via
//! patternlanguage.cc/Patterns/Quiet-Backs-(59):
//! > **Problem:** Anyone who has to work in noise, in offices with people
//! > all around, needs to be able to pause and refresh themselves with
//! > quiet in a more natural situation.
//! > **Solution:** Give the buildings in the busy parts of town a quiet
//! > 'back' behind them and away from the noise.
//!
//! # A real, checkable proxy
//!
//! "Busy" collapses to `Street.classification == "arterial"`, the only
//! real proxy for traffic volume this schema has. For each building whose
//! nearest wall vertex sits within `FRONT_THRESHOLD_M` (15m) of an
//! arterial street (i.e. it genuinely fronts one), this opinion checks
//! whether the building's OWN FARTHEST wall vertex clears
//! `QUIET_THRESHOLD_M` (40m) from every arterial street -- a real,
//! computable front-to-back distance gradient away from traffic, using
//! only real `Street.classification` and `Building.polygon` data.
//! `value` = fraction of arterial-fronting buildings with a real quiet
//! back.

use std::collections::BTreeMap;
use street_smarts_core::geometry::point_to_polyline_m;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P59QuietBacks;

const FRONT_THRESHOLD_M: f64 = 15.0;
const QUIET_THRESHOLD_M: f64 = 40.0;

impl Opinion for P59QuietBacks {
    fn name(&self) -> &'static str {
        "p59_quiet_backs"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p59".into(),
            display: "Alexander et al., A Pattern Language, Pattern 59 (Quiet Backs)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Quiet-Backs-(59)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let arterials: Vec<_> = n.streets.iter().filter(|s| s.classification.as_deref() == Some("arterial")).collect();
        if arterials.is_empty() {
            return OpinionOutput::NoView {
                reason: "No arterial-classified street in this neighborhood -- nothing 'busy' to \
                         check a quiet back against."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut fronting = 0usize;
        let mut has_quiet_back = 0usize;
        let mut noisy: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for b in &n.buildings {
            if b.polygon.outer.len() < 3 {
                continue;
            }
            let (mut front_m, mut back_m) = (f64::MAX, f64::MIN);
            for v in &b.polygon.outer {
                let d = arterials.iter().map(|s| point_to_polyline_m(v, &s.centerline)).fold(f64::MAX, f64::min);
                front_m = front_m.min(d);
                back_m = back_m.max(d);
            }
            if front_m > FRONT_THRESHOLD_M {
                continue; // doesn't front an arterial -- not applicable
            }
            fronting += 1;
            let ok = back_m >= QUIET_THRESHOLD_M;
            details.insert(format!("{}.front_m", b.id), format!("{front_m:.1}"));
            details.insert(format!("{}.back_m", b.id), format!("{back_m:.1}"));
            if ok {
                has_quiet_back += 1;
            } else {
                noisy.push(b.id.clone());
            }
        }

        if fronting == 0 {
            return OpinionOutput::NoView {
                reason: "No building sits within the front-adjacency threshold of an arterial street.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let value = has_quiet_back as f64 / fronting as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("quiet_back_fraction".into(), value);
        details.insert("n_arterial_fronting_buildings".into(), fronting.to_string());
        details.insert("front_threshold_m".into(), format!("{FRONT_THRESHOLD_M:.0}"));
        details.insert("quiet_threshold_m".into(), format!("{QUIET_THRESHOLD_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} building(s) front a real arterial street; {:.0}% have a far wall that clears \
                 {:.0}m from every arterial -- a real quiet back.",
                fronting, value * 100.0, QUIET_THRESHOLD_M
            ),
            sub_scores,
            details,
            caveats: vec![
                "'Busy' collapses to Street.classification == \"arterial\" -- no traffic volume, \
                 noise level, or foot-traffic concept exists in this schema.".into(),
                "Checks distance only, not whether the real text's own 'walk along the quiet back, \
                 protected by walls and distance' path actually exists -- no path/walk-network \
                 concept at this scale is checked here.".into(),
                "Doesn't confirm the quiet-back area isn't itself a shortcut for busy foot traffic, \
                 as the real text also requires.".into(),
            ],
            contributing_features: noisy,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{Building, NeighborhoodMeta, Street};

    fn nbhd(buildings: Vec<Building>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings, streets, open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P59 unit fixture".into(),
            },
        }
    }

    fn arterial() -> Street {
        let m = 1.0 / 111_320.0;
        Street { id: "S1".into(), centerline: vec![LngLat::new(-200.0 * m, 0.0), LngLat::new(200.0 * m, 0.0)], classification: Some("arterial".into()), row_width_m: Some(18.0), surface: None }
    }

    fn building(id: &str, y0: f64, y1: f64) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(-5.0 * m, y0 * m), LngLat::new(5.0 * m, y0 * m),
                LngLat::new(5.0 * m, y1 * m), LngLat::new(-5.0 * m, y1 * m),
                LngLat::new(-5.0 * m, y0 * m),
            ]),
            height_m: Some(9.0), typology: None, year_built: None, parcel_id: None, floors: None,
            openings: vec![], interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        }
    }

    #[test]
    fn no_arterials_is_no_view() {
        assert!(matches!(P59QuietBacks.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_deep_building_with_a_far_back_scores_full() {
        // Fronts the arterial at y=10, back wall at y=60 -- 50m clears the 40m quiet threshold.
        let n = nbhd(vec![building("B1", 10.0, 60.0)], vec![arterial()]);
        match P59QuietBacks.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_shallow_building_with_no_quiet_back_is_flagged() {
        // Fronts the arterial at y=10, back wall at y=20 -- only 20m, under the 40m threshold.
        let n = nbhd(vec![building("B1", 10.0, 20.0)], vec![arterial()]);
        match P59QuietBacks.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-6, "got {value}");
                assert!(contributing_features.contains(&"B1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
