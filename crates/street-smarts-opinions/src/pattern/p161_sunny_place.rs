//! P161 Sunny Place — every building needs a real, wind-protected sunny
//! spot immediately outside it, at least 6 feet deep.
//!
//! From Alexander, *A Pattern Language*, Pattern 161 (p. 757), via
//! patternlanguage.cc/Patterns/Sunny-Place-(161):
//! > **Problem:** The area immediately outside the building, to the
//! > south -- that angle between its walls and the earth where the sun
//! > falls -- must be developed and made into a place which lets people
//! > bask in it.
//! > **Solution:** Find the south-facing spot between building and
//! > outdoors with optimal sun exposure and develop it as a special
//! > outdoor room... The space should function as a defined room -- at
//! > minimum six feet deep.
//!
//! # A real, checkable proxy
//!
//! Same real south-of-the-building orientation check
//! `p105_south_facing_outdoors.rs` already establishes (building centroid
//! at a higher latitude than the space's own centroid, real adjacency
//! within threshold), PLUS the real "at least six feet deep" dimensional
//! claim P105 doesn't check: the open space's own local bounding-box
//! short side must clear `MIN_DEPTH_M` (1.83m, Alexander's literal 6ft).
//!
//! `value = 0.5 * (P105's own south+adjacency check) + 0.5 *
//! min_depth_fraction`. Cannot check "wind-protected" -- no wind/massing-
//! shelter simulation exists in this pipeline at all.

use std::collections::BTreeMap;
use street_smarts_core::geometry::{haversine_m, LngLat};
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P161SunnyPlace;

const ADJACENCY_THRESHOLD_M: f64 = 30.0;
const MIN_DEPTH_M: f64 = 1.83; // 6 feet

fn bbox_short_side_m(ring: &[LngLat]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let lat0 = ring.iter().map(|p| p.lat).sum::<f64>() / ring.len() as f64;
    let mlat = lat0.to_radians().cos();
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in ring {
        let x = p.lng * mlat * 111_320.0;
        let y = p.lat * 110_540.0;
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    (max_x - min_x).min(max_y - min_y)
}

impl Opinion for P161SunnyPlace {
    fn name(&self) -> &'static str {
        "p161_sunny_place"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p161".into(),
            display: "Alexander et al., A Pattern Language, Pattern 161 (Sunny Place)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Sunny-Place-(161)".into()),
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
                reason: "No buildings to check sunny-place orientation against yet -- run P107 Wings of Light first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut orientation_flags: Vec<bool> = Vec::new();
        let mut depth_flags: Vec<bool> = Vec::new();
        let mut weak: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for o in &spaces {
            let oc = o.polygon.centroid();
            let (nearest_b, nearest_dist) = n.buildings.iter()
                .map(|b| (b, haversine_m(&oc, &b.polygon.centroid())))
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                .unwrap();
            let bc = nearest_b.polygon.centroid();
            let is_south_of_north_building = bc.lat > oc.lat && nearest_dist <= ADJACENCY_THRESHOLD_M;
            let depth = bbox_short_side_m(&o.polygon.outer);
            let deep_enough = depth >= MIN_DEPTH_M;
            orientation_flags.push(is_south_of_north_building);
            depth_flags.push(deep_enough);
            details.insert(format!("{}.depth_m", o.id), format!("{depth:.1}"));
            if !is_south_of_north_building || !deep_enough {
                weak.push(o.id.clone());
            }
        }

        let orientation_fraction = orientation_flags.iter().filter(|f| **f).count() as f64 / orientation_flags.len() as f64;
        let depth_fraction = depth_flags.iter().filter(|f| **f).count() as f64 / depth_flags.len() as f64;
        let value = 0.5 * orientation_fraction + 0.5 * depth_fraction;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("south_orientation_fraction".into(), orientation_fraction);
        sub_scores.insert("min_depth_fraction".into(), depth_fraction);
        details.insert("n_open_space".into(), spaces.len().to_string());
        details.insert("min_depth_m".into(), format!("{MIN_DEPTH_M:.2}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} resolved open-space feature(s); {:.0}% sit south of (and close to) a real \
                 building, {:.0}% clear Alexander's literal 6ft ({MIN_DEPTH_M:.2}m) depth minimum.",
                spaces.len(), orientation_fraction * 100.0, depth_fraction * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Cannot check 'wind-protected' -- no wind or massing-shelter simulation exists in \
                 this pipeline at all.".into(),
                "Same orientation-proxy limits as p105_south_facing_outdoors.rs: assumes the \
                 northern hemisphere, centroid-to-centroid only, doesn't verify a real facade \
                 relationship.".into(),
                "Depth is a bounding-box short-side proxy, not a true 'defined room' shape check -- \
                 an irregular sliver could have a misleading bbox short side.".into(),
            ],
            contributing_features: weak,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{Building, NeighborhoodMeta, OpenSpace};

    fn nbhd(open_space: Vec<OpenSpace>, buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![], open_space,
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P161 unit fixture".into(),
            },
        }
    }

    fn rect_ring(cx: f64, cy: f64, w: f64, h: f64) -> Vec<LngLat> {
        let m = 1.0 / 111_320.0;
        vec![
            LngLat::new((cx - w / 2.0) * m, (cy - h / 2.0) * m), LngLat::new((cx + w / 2.0) * m, (cy - h / 2.0) * m),
            LngLat::new((cx + w / 2.0) * m, (cy + h / 2.0) * m), LngLat::new((cx - w / 2.0) * m, (cy + h / 2.0) * m),
        ]
    }

    fn building_at(id: &str, cx: f64, cy: f64) -> Building {
        Building {
            id: id.into(), polygon: Polygon::from_ring(rect_ring(cx, cy, 10.0, 10.0)),
            height_m: Some(9.0), typology: None, year_built: None, parcel_id: None,
            floors: None, openings: vec![], interior_cells: vec![],
        }
    }

    #[test]
    fn no_open_space_is_no_view() {
        assert!(matches!(P161SunnyPlace.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_deep_sunny_space_south_of_its_building_scores_full() {
        let space = OpenSpace { id: "S1".into(), polygon: Polygon::from_ring(rect_ring(0.0, 0.0, 6.0, 6.0)), kind: OpenSpaceKind::Plaza };
        let n = nbhd(vec![space], vec![building_at("B1", 0.0, 15.0)]);
        let out = P161SunnyPlace.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["south_orientation_fraction"] - 1.0).abs() < 1e-6);
                assert!((sub_scores["min_depth_fraction"] - 1.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_token_sliver_fails_the_depth_check() {
        // 1m x 6m -- short side well under the 1.83m (6ft) minimum.
        let space = OpenSpace { id: "S1".into(), polygon: Polygon::from_ring(rect_ring(0.0, 0.0, 1.0, 6.0)), kind: OpenSpaceKind::Plaza };
        let n = nbhd(vec![space], vec![building_at("B1", 0.0, 15.0)]);
        let out = P161SunnyPlace.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["min_depth_fraction"] - 0.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
