//! P105 South Facing Outdoors — open space should sit on the sunny south
//! side of its building, not shadowed to the north.
//!
//! From Alexander, *A Pattern Language*, Pattern 105 (p. 513), via
//! patternlanguage.cc/Patterns/South-Facing-Outdoors-(105):
//! > **Problem:** People use open space if it is sunny, and do not use it
//! > if it isn't, in all but desert climates.
//! > **Solution:** Always place buildings to the north of the outdoor
//! > spaces that go with them, and keep the outdoor spaces to the south.
//! > Never leave a deep band of shade between the building and the sunny
//! > part of the outdoors.
//!
//! # Real, checkable geometry -- assumes the northern hemisphere
//!
//! This fixture set (Norfolk, VA) is in the northern hemisphere, where
//! "south-facing" and "sunny" correlate the way Alexander's text assumes;
//! this opinion checks `building.lat > open_space.lat` (the building sits
//! at a higher latitude, i.e. north of the space) accordingly. It would
//! give backwards answers run against a real southern-hemisphere site --
//! not something this codebase's fixtures currently exercise, but a real
//! limit worth naming rather than silently assuming global validity.
//!
//! # Two sub-scores
//!
//! - `building_is_north`: fraction of resolved open-space features whose
//!   nearest building's centroid sits at a higher latitude (further
//!   north) than the space's own centroid.
//! - `close_adjacency`: fraction where that nearest building is within
//!   `adjacency_threshold_m` (default 30m) -- a real proxy for "the
//!   outdoor space that goes with" a specific building (the pattern's own
//!   premise), not an unrelated open space that happens to be north-south
//!   aligned by coincidence.
//!
//! `value = 0.5 * building_is_north + 0.5 * close_adjacency`.

use std::collections::BTreeMap;
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P105SouthFacingOutdoors;

const ADJACENCY_THRESHOLD_M: f64 = 30.0;

impl Opinion for P105SouthFacingOutdoors {
    fn name(&self) -> &'static str {
        "p105_south_facing_outdoors"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p105".into(),
            display: "Alexander et al., A Pattern Language, Pattern 105 (South Facing Outdoors)".into(),
            url: Some("https://patternlanguage.cc/Patterns/South-Facing-Outdoors-(105)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let spaces: Vec<_> = n.open_space.iter().filter(|o| o.kind != OpenSpaceKind::Undecided && o.polygon.area_m2() > 0.0).collect();
        if spaces.is_empty() {
            return OpinionOutput::NoView {
                reason: "No resolved open space in this neighborhood.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }
        if n.buildings.is_empty() {
            return OpinionOutput::NoView {
                reason: "No buildings to check open space against yet -- run P107 Wings of Light first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut north_flags: Vec<bool> = Vec::new();
        let mut close_flags: Vec<bool> = Vec::new();
        let mut not_south: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for o in &spaces {
            let oc = o.polygon.centroid();
            let (nearest_b, nearest_dist) = n.buildings.iter()
                .map(|b| (b, haversine_m(&oc, &b.polygon.centroid())))
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                .unwrap();
            let bc = nearest_b.polygon.centroid();
            let is_north = bc.lat > oc.lat;
            north_flags.push(is_north);
            close_flags.push(nearest_dist <= ADJACENCY_THRESHOLD_M);
            details.insert(format!("{}.nearest_building_dist_m", o.id), format!("{nearest_dist:.0}"));
            if !is_north {
                not_south.push(o.id.clone());
            }
        }

        let building_is_north = north_flags.iter().filter(|f| **f).count() as f64 / north_flags.len() as f64;
        let close_adjacency = close_flags.iter().filter(|f| **f).count() as f64 / close_flags.len() as f64;
        let value = 0.5 * building_is_north + 0.5 * close_adjacency;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("building_is_north".into(), building_is_north);
        sub_scores.insert("close_adjacency".into(), close_adjacency);

        details.insert("n_open_space".into(), spaces.len().to_string());
        details.insert("adjacency_threshold_m".into(), format!("{ADJACENCY_THRESHOLD_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} resolved open-space feature(s); {:.0}% have their nearest building to the \
                 north (sunny-south orientation); {:.0}% are within {:.0}m of that building.",
                spaces.len(), building_is_north * 100.0, close_adjacency * 100.0, ADJACENCY_THRESHOLD_M
            ),
            sub_scores,
            details,
            caveats: vec![
                "Assumes the northern hemisphere -- see this module's own doc comment. Would give \
                 backwards answers on a southern-hemisphere site.".into(),
                "Centroid-to-centroid latitude comparison only -- doesn't check for a real 'deep \
                 band of shade' (an actual shadow/solar-angle simulation), and doesn't verify the \
                 open space genuinely lies along the building's south FACADE rather than merely \
                 somewhere south of its centroid.".into(),
                "'Nearest building' may not be the building the open space was actually designed \
                 to serve -- no explicit pairing data exists between a courtyard/plaza and its \
                 intended building.".into(),
            ],
            contributing_features: not_south,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{Building, NeighborhoodMeta, OpenSpace};

    fn nbhd(open_space: Vec<OpenSpace>, buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![], open_space,
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P105 unit fixture".into(),
            },
        }
    }

    fn square_at(id_prefix: &str, x_m: f64, y_m: f64) -> (OpenSpace, Building) {
        let m = 1.0 / 111_320.0;
        let half = 5.0;
        let ring = |cx: f64, cy: f64| vec![
            LngLat::new((cx - half) * m, (cy - half) * m), LngLat::new((cx + half) * m, (cy - half) * m),
            LngLat::new((cx + half) * m, (cy + half) * m), LngLat::new((cx - half) * m, (cy + half) * m),
            LngLat::new((cx - half) * m, (cy - half) * m),
        ];
        let space = OpenSpace { id: format!("{id_prefix}_SPACE"), polygon: Polygon::from_ring(ring(x_m, y_m)), kind: OpenSpaceKind::Plaza };
        let building = Building {
            id: format!("{id_prefix}_BLDG"),
            polygon: Polygon::from_ring(ring(x_m, y_m + 20.0)), // 20m north of the space
            height_m: Some(9.0), typology: Some("p107_solid_v01".into()),
            year_built: None, parcel_id: None, floors: Some(3), openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
        };
        (space, building)
    }

    #[test]
    fn no_open_space_is_no_view() {
        assert!(matches!(P105SouthFacingOutdoors.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn building_correctly_north_of_its_space_scores_high() {
        let (space, building) = square_at("PAIR", 0.0, 0.0);
        let n = nbhd(vec![space], vec![building]);
        let out = P105SouthFacingOutdoors.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["building_is_north"] - 1.0).abs() < 1e-6);
                assert!((sub_scores["close_adjacency"] - 1.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn building_south_of_its_space_is_flagged() {
        let m = 1.0 / 111_320.0;
        let half = 5.0;
        let ring = |cx: f64, cy: f64| vec![
            LngLat::new((cx - half) * m, (cy - half) * m), LngLat::new((cx + half) * m, (cy - half) * m),
            LngLat::new((cx + half) * m, (cy + half) * m), LngLat::new((cx - half) * m, (cy + half) * m),
            LngLat::new((cx - half) * m, (cy - half) * m),
        ];
        let space = OpenSpace { id: "S1".into(), polygon: Polygon::from_ring(ring(0.0, 20.0)), kind: OpenSpaceKind::Plaza };
        let building = Building {
            id: "B1".into(), polygon: Polygon::from_ring(ring(0.0, 0.0)), // south of the space -- wrong
            height_m: Some(9.0), typology: Some("p107_solid_v01".into()),
            year_built: None, parcel_id: None, floors: Some(3), openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
        };
        let n = nbhd(vec![space], vec![building]);
        let out = P105SouthFacingOutdoors.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, contributing_features, .. } => {
                assert!((sub_scores["building_is_north"] - 0.0).abs() < 1e-6);
                assert!(contributing_features.contains(&"S1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_distant_north_building_fails_close_adjacency() {
        let m = 1.0 / 111_320.0;
        let half = 5.0;
        let ring = |cx: f64, cy: f64| vec![
            LngLat::new((cx - half) * m, (cy - half) * m), LngLat::new((cx + half) * m, (cy - half) * m),
            LngLat::new((cx + half) * m, (cy + half) * m), LngLat::new((cx - half) * m, (cy + half) * m),
            LngLat::new((cx - half) * m, (cy - half) * m),
        ];
        let space = OpenSpace { id: "S1".into(), polygon: Polygon::from_ring(ring(0.0, 0.0)), kind: OpenSpaceKind::Plaza };
        let building = Building {
            id: "B1".into(), polygon: Polygon::from_ring(ring(0.0, 500.0)), // far north
            height_m: Some(9.0), typology: Some("p107_solid_v01".into()),
            year_built: None, parcel_id: None, floors: Some(3), openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
        };
        let n = nbhd(vec![space], vec![building]);
        let out = P105SouthFacingOutdoors.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["building_is_north"] - 1.0).abs() < 1e-6);
                assert!((sub_scores["close_adjacency"] - 0.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
