//! P122 Building Fronts — buildings should come right up to the paths
//! that front them, not sit behind a setback.
//!
//! From Alexander, *A Pattern Language*, Pattern 122, via
//! patternlanguage.cc/Patterns/Building-Fronts-(122):
//! > **Solution:** On no account allow set-backs between streets or paths
//! > or public open land and the buildings which front on them... Build
//! > right up to the paths.
//!
//! # A real, checkable gap or non-gap -- checked, not assumed
//!
//! `p107_wings_of_light`'s own `setback_m` defaults to 3.0m, applied to
//! any pad that didn't already come from a P95 pad (which reserves its
//! own gap via `pad_inset_m` instead -- see `p107_wings_of_light.rs`'s
//! own doc comment on why the two aren't stacked). Whether a REAL
//! pipeline building actually ends up set back from its nearest street
//! depends on which path it took; this opinion measures the real gap
//! rather than assuming either outcome.
//!
//! `value` = fraction of buildings whose nearest real street sits within
//! `max_setback_m` (default 1.0m -- "right up to the paths," a small
//! tolerance for real corner rounding, not a real setback allowance).

use std::collections::BTreeMap;
use street_smarts_core::geometry::{haversine_m, point_to_polyline_m};
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P122BuildingFronts;

const MAX_SETBACK_M: f64 = 1.0;

impl Opinion for P122BuildingFronts {
    fn name(&self) -> &'static str {
        "p122_building_fronts"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p122".into(),
            display: "Alexander et al., A Pattern Language, Pattern 122 (Building Fronts)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Building-Fronts-(122)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        if n.buildings.is_empty() || n.streets.is_empty() {
            return OpinionOutput::NoView {
                reason: "Needs both real buildings and real streets to measure setback.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let streets_with_geometry: Vec<_> = n.streets.iter().filter(|s| s.centerline.len() >= 2).collect();
        if streets_with_geometry.is_empty() {
            return OpinionOutput::NoView {
                reason: "No street has real centerline geometry to measure against.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut setbacks: Vec<f64> = Vec::new();
        let mut set_back: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for b in &n.buildings {
            let bc = b.polygon.centroid();
            let nearest_wall = b.polygon.outer.iter().map(|p| haversine_m(&bc, p)).fold(0.0_f64, f64::max);
            let nearest_street = streets_with_geometry.iter().map(|s| point_to_polyline_m(&bc, &s.centerline)).fold(f64::MAX, f64::min);
            let setback = (nearest_street - nearest_wall).max(0.0);
            setbacks.push(setback);
            details.insert(format!("{}.setback_m", b.id), format!("{setback:.2}"));
            if setback > MAX_SETBACK_M {
                set_back.push(b.id.clone());
            }
        }

        let value = setbacks.iter().filter(|s| **s <= MAX_SETBACK_M).count() as f64 / setbacks.len() as f64;
        let mean_setback = setbacks.iter().sum::<f64>() / setbacks.len() as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("no_setback_fraction".into(), value);
        sub_scores.insert("mean_setback_m".into(), mean_setback);
        details.insert("n_buildings".into(), setbacks.len().to_string());
        details.insert("max_setback_threshold_m".into(), format!("{MAX_SETBACK_M:.1}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} building(s); mean setback from nearest street {:.2}m; {:.0}% build right up to \
                 the path (<= {:.1}m).",
                setbacks.len(), mean_setback, value * 100.0, MAX_SETBACK_M
            ),
            sub_scores,
            details,
            caveats: vec![
                "Setback is estimated as (distance from building centroid to nearest street point) \
                 minus (centroid-to-farthest-wall-vertex distance) -- a coarse radius-based proxy, \
                 not a real perpendicular gap measurement to the nearest actual wall segment.".into(),
                "'Nearest street point' includes every centerline vertex, not just the segment \
                 that genuinely fronts the building -- a building near a street corner could read a \
                 smaller gap than its real frontage has.".into(),
            ],
            contributing_features: set_back,
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
                layer_provenance: Default::default(), label: "P122 unit fixture".into(),
            },
        }
    }

    fn square_building(id: &str, half: f64) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![LngLat::new(-half * m, -half * m), LngLat::new(half * m, -half * m), LngLat::new(half * m, half * m), LngLat::new(-half * m, half * m), LngLat::new(-half * m, -half * m)]),
            height_m: Some(6.0), typology: Some("p107_solid_v01".into()), year_built: None, parcel_id: None, floors: Some(2), openings: vec![], interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        }
    }

    fn street_at(dist_m: f64) -> Street {
        let m = 1.0 / 111_320.0;
        Street { id: "S1".into(), centerline: vec![LngLat::new(-50.0 * m, dist_m * m), LngLat::new(50.0 * m, dist_m * m)], classification: Some("local".into()), row_width_m: Some(4.0), surface: None }
    }

    #[test]
    fn no_buildings_or_streets_is_no_view() {
        assert!(matches!(P122BuildingFronts.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_building_right_up_to_the_street_scores_full_credit() {
        // 10m half-width building, street 10m from center -> ~0m setback.
        let n = nbhd(vec![square_building("B1", 10.0)], vec![street_at(10.0)]);
        let out = P122BuildingFronts.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_building_set_back_from_the_street_is_flagged() {
        // 10m half-width building, street 30m from center -> ~20m real gap.
        let n = nbhd(vec![square_building("B1", 10.0)], vec![street_at(30.0)]);
        let out = P122BuildingFronts.evaluate(&n);
        match out {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-6);
                assert!(contributing_features.contains(&"B1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
