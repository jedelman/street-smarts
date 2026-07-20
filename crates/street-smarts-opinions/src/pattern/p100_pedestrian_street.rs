//! P100 Pedestrian Street — a pedestrian street needs many real entrances
//! along it, not a blank wall punctuated by one or two doors.
//!
//! From Alexander, *A Pattern Language*, Pattern 100, via
//! patternlanguage.cc/Patterns/Pedestrian-Street-(100):
//! > **Solution:** Arrange buildings so that they form pedestrian streets
//! > with many entrances and open stairs directly from the upper stories
//! > to the street, so that even movement between rooms is outdoors, not
//! > just movement between buildings.
//!
//! # A real, checkable entrance-density proxy
//!
//! For each `Pedestrian`-classified street, this opinion counts real
//! ground-floor entrances (`Opening { kind: Door, on_hole: false, floor:
//! 0 }`) within `adjacency_threshold_m` (default 20m), then checks that
//! density against `min_entrances_per_100m` (default 3 -- a defensible,
//! not Alexander-sourced, walkable-frontage target).
//!
//! Doesn't check "open stairs directly from upper stories to the
//! street" -- no vertical-circulation-to-exterior data exists in this
//! schema (see `InteriorCell.floor`'s own doc comment: "Always 0 in this
//! version").

use std::collections::BTreeMap;
use street_smarts_core::geometry::{haversine_m, point_to_polyline_m};
use street_smarts_core::nir::{Neighborhood, OpeningKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P100PedestrianStreet;

const ADJACENCY_THRESHOLD_M: f64 = 20.0;
const MIN_ENTRANCES_PER_100M: f64 = 3.0;

impl Opinion for P100PedestrianStreet {
    fn name(&self) -> &'static str {
        "p100_pedestrian_street"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p100".into(),
            display: "Alexander et al., A Pattern Language, Pattern 100 (Pedestrian Street)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Pedestrian-Street-(100)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let ped_streets: Vec<_> = n.streets.iter().filter(|s| s.classification.as_deref() == Some("pedestrian")).collect();
        if ped_streets.is_empty() {
            return OpinionOutput::NoView {
                reason: "No Pedestrian-classified streets in this neighborhood.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let entrances: Vec<street_smarts_core::geometry::LngLat> = n.buildings.iter()
            .filter_map(|b| b.openings.iter().find(|o| o.kind == OpeningKind::Door && !o.on_hole && o.floor == 0).map(|d| {
                let ring = &b.polygon.outer;
                let idx = d.ring_index.min(ring.len().saturating_sub(2));
                let a = ring[idx];
                let c = ring[(idx + 1) % ring.len()];
                street_smarts_core::geometry::LngLat::new(a.lng + (c.lng - a.lng) * d.t, a.lat + (c.lat - a.lat) * d.t)
            }))
            .collect();

        let mut density_scores: Vec<f64> = Vec::new();
        let mut sparse: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for s in &ped_streets {
            let street_len: f64 = s.centerline.windows(2).map(|w| haversine_m(&w[0], &w[1])).sum();
            if street_len <= 0.0 {
                continue;
            }
            let n_nearby = entrances.iter()
                .filter(|e| point_to_polyline_m(e, &s.centerline) <= ADJACENCY_THRESHOLD_M)
                .count();
            let per_100m = n_nearby as f64 / street_len * 100.0;
            let score = (per_100m / MIN_ENTRANCES_PER_100M).clamp(0.0, 1.0);
            density_scores.push(score);
            details.insert(format!("{}.entrances_per_100m", s.id), format!("{per_100m:.1}"));
            if score < 1.0 {
                sparse.push(s.id.clone());
            }
        }

        if density_scores.is_empty() {
            return OpinionOutput::NoView {
                reason: "No Pedestrian street had positive real length to measure.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let value = density_scores.iter().sum::<f64>() / density_scores.len() as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("entrance_density".into(), value);
        details.insert("n_pedestrian_streets".into(), ped_streets.len().to_string());
        details.insert("min_entrances_per_100m".into(), format!("{MIN_ENTRANCES_PER_100M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} Pedestrian street(s); mean entrance density {:.2} relative to the {:.0}-per-100m target.",
                ped_streets.len(), value, MIN_ENTRANCES_PER_100M
            ),
            sub_scores,
            details,
            caveats: vec![
                "Doesn't check 'open stairs directly from upper stories to the street' -- no \
                 exterior vertical-circulation data exists in this schema.".into(),
                "min_entrances_per_100m (3) is a defensible walkable-frontage target, not a number \
                 Alexander's own text gives.".into(),
            ],
            contributing_features: sparse,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{Building, NeighborhoodMeta, Opening, Street};

    fn nbhd(buildings: Vec<Building>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings, streets, open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P100 unit fixture".into(),
            },
        }
    }

    fn ped_street(len_m: f64) -> Street {
        let m = 1.0 / 111_320.0;
        Street { id: "PED1".into(), centerline: vec![LngLat::new(0.0, -5.0 * m), LngLat::new(len_m * m, -5.0 * m)], classification: Some("pedestrian".into()), row_width_m: Some(4.0), surface: None }
    }

    fn building_with_door(id: &str, x_m: f64) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![LngLat::new(x_m * m, 0.0), LngLat::new((x_m + 5.0) * m, 0.0), LngLat::new((x_m + 5.0) * m, 5.0 * m), LngLat::new(x_m * m, 5.0 * m), LngLat::new(x_m * m, 0.0)]),
            height_m: Some(6.0), typology: Some("p107_solid_v01".into()), year_built: None, parcel_id: None, floors: Some(2),
            openings: vec![Opening { kind: OpeningKind::Door, ring_index: 0, on_hole: false, t: 0.5, width_m: 0.9, sill_height_m: 0.0, head_height_m: 2.1, floor: 0 }],
            interior_cells: vec![],
            wall_thickness_m: None,
        }
    }

    #[test]
    fn no_pedestrian_streets_is_no_view() {
        assert!(matches!(P100PedestrianStreet.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn dense_entrances_score_high() {
        // 100m street, 4 buildings with doors along it -> 4 per 100m > target of 3.
        let buildings: Vec<Building> = (0..4).map(|i| building_with_door(&format!("B{i}"), i as f64 * 20.0)).collect();
        let n = nbhd(buildings, vec![ped_street(100.0)]);
        let out = P100PedestrianStreet.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn sparse_entrances_score_low() {
        let n = nbhd(vec![building_with_door("B1", 0.0)], vec![ped_street(500.0)]);
        let out = P100PedestrianStreet.evaluate(&n);
        match out {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!(value < 0.3, "got {value}");
                assert!(contributing_features.contains(&"PED1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
