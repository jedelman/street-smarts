//! P192 Windows Overlooking Life — a window should look out over real
//! activity (a street, a garden, open space), not stare into a blank wall
//! a few feet away.
//!
//! From Alexander, *A Pattern Language*, Pattern 192, via
//! patternlanguage.cc/Patterns/Windows-Overlooking-Life-(192):
//! > **Problem:** Rooms without a view are prisons for the people who
//! > have to stay in them.
//! > **Solution:** ...place [windows] to give the best possible views of
//! > life around the building: the garden, street, or anything which
//! > contrasts with the indoor scene, to people inside the building.
//!
//! # A real, checkable proxy -- proximity, not verified sightline
//!
//! For each ground-floor, outer-wall window (`Opening { kind: Window,
//! on_hole: false, floor: 0 }`), this opinion computes its exterior point
//! (the same ring-interpolation `p100_pedestrian_street` and
//! `p112_entrance_transition` use) and checks:
//!
//! - `has_life_nearby`: a real street (perpendicular distance to its
//!   nearest segment, not just a vertex) or a resolved (non-`Undecided`)
//!   open-space centroid sits within `view_threshold_m` (default 25m).
//! - `not_blocked`: no OTHER building's wall vertex sits within
//!   `blocked_threshold_m` (default 6m) -- a real, close neighbor reading
//!   as a blank wall rather than "life."
//!
//! `value` = fraction of windows where both hold.
//!
//! Cannot check the 25%-of-floor-area glazing target the real text gives
//! for the San Francisco Bay Area (no floor-area-to-glazing ratio exists
//! in this schema), and doesn't verify an actual sightline -- proximity to
//! a street/open space is a real but coarse stand-in for "the view out
//! this window is actually of that thing."

use std::collections::BTreeMap;
use street_smarts_core::geometry::{haversine_m, point_to_polyline_m, LngLat};
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind, OpeningKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P192WindowsOverlookingLife;

const VIEW_THRESHOLD_M: f64 = 25.0;
const BLOCKED_THRESHOLD_M: f64 = 6.0;

fn opening_point(ring: &[LngLat], ring_index: usize, t: f64) -> Option<LngLat> {
    if ring.len() < 2 {
        return None;
    }
    let idx = ring_index.min(ring.len().saturating_sub(2));
    let a = ring[idx];
    let c = ring[(idx + 1) % ring.len()];
    Some(LngLat::new(a.lng + (c.lng - a.lng) * t, a.lat + (c.lat - a.lat) * t))
}

impl Opinion for P192WindowsOverlookingLife {
    fn name(&self) -> &'static str {
        "p192_windows_overlooking_life"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p192".into(),
            display: "Alexander et al., A Pattern Language, Pattern 192 (Windows Overlooking Life)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Windows-Overlooking-Life-(192)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let open_space_centroids: Vec<LngLat> = n.open_space.iter()
            .filter(|o| o.kind != OpenSpaceKind::Undecided && o.polygon.area_m2() > 0.0)
            .map(|o| o.polygon.centroid())
            .collect();

        let mut checked = 0usize;
        let mut overlooks_life = 0usize;
        let mut blocked: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for (bi, b) in n.buildings.iter().enumerate() {
            for (wi, w) in b.openings.iter().enumerate().filter(|(_, w)| w.kind == OpeningKind::Window && !w.on_hole && w.floor == 0) {
                let Some(p) = opening_point(&b.polygon.outer, w.ring_index, w.t) else { continue };
                checked += 1;

                let nearest_life = n.streets.iter().map(|s| point_to_polyline_m(&p, &s.centerline))
                    .chain(open_space_centroids.iter().map(|q| haversine_m(&p, q)))
                    .fold(f64::MAX, f64::min);
                let nearest_other_building = n.buildings.iter().enumerate()
                    .filter(|(oi, _)| *oi != bi)
                    .flat_map(|(_, ob)| ob.polygon.outer.iter())
                    .map(|q| haversine_m(&p, q))
                    .fold(f64::MAX, f64::min);

                let has_life = nearest_life <= VIEW_THRESHOLD_M;
                let not_blocked = nearest_other_building > BLOCKED_THRESHOLD_M;
                let ok = has_life && not_blocked;
                let key = format!("{}.window[{}]", b.id, wi);
                details.insert(format!("{key}.nearest_life_m"), format!("{:.1}", nearest_life.min(9999.0)));
                details.insert(format!("{key}.nearest_neighbor_m"), format!("{:.1}", nearest_other_building.min(9999.0)));
                if ok {
                    overlooks_life += 1;
                } else {
                    blocked.push(key);
                }
            }
        }

        if checked == 0 {
            return OpinionOutput::NoView {
                reason: "No ground-floor, outer-wall window openings to check yet -- run \
                         P221 Natural Doors and Windows first."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let value = overlooks_life as f64 / checked as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("overlooks_life_fraction".into(), value);
        details.insert("n_windows_checked".into(), checked.to_string());
        details.insert("view_threshold_m".into(), format!("{VIEW_THRESHOLD_M:.0}"));
        details.insert("blocked_threshold_m".into(), format!("{BLOCKED_THRESHOLD_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} ground-floor window(s) checked; {:.0}% have real street/open-space activity \
                 within {:.0}m and no neighboring building within {:.0}m.",
                checked, value * 100.0, VIEW_THRESHOLD_M, BLOCKED_THRESHOLD_M
            ),
            sub_scores,
            details,
            caveats: vec![
                "Cannot check the real text's 25%-of-floor-area glazing target -- no floor-area-\
                 to-glazing ratio exists in this schema.".into(),
                "Proximity to a street or open space is a coarse stand-in for a verified \
                 sightline -- doesn't confirm the window's own facing direction actually looks \
                 toward that feature.".into(),
                "'Life' includes any resolved open space, even a quiet Sponge or Vacant-turned-\
                 Park -- not confirmed to have real pedestrian activity.".into(),
            ],
            contributing_features: blocked,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{Building, NeighborhoodMeta, Opening, Street};

    fn nbhd(buildings: Vec<Building>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings, streets, open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P192 unit fixture".into(),
            },
        }
    }

    fn building_with_window(id: &str, x_m: f64, y_m: f64) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(x_m * m, y_m * m), LngLat::new((x_m + 6.0) * m, y_m * m),
                LngLat::new((x_m + 6.0) * m, (y_m + 6.0) * m), LngLat::new(x_m * m, (y_m + 6.0) * m),
                LngLat::new(x_m * m, y_m * m),
            ]),
            height_m: Some(6.0), typology: Some("p107_solid_v01".into()), year_built: None,
            parcel_id: None, floors: Some(2),
            openings: vec![Opening { kind: OpeningKind::Window, ring_index: 0, on_hole: false, t: 0.5, width_m: 1.2, sill_height_m: 0.9, head_height_m: 2.1, floor: 0 }],
            interior_cells: vec![],
        }
    }

    fn street_at(y_m: f64) -> Street {
        let m = 1.0 / 111_320.0;
        Street { id: "S1".into(), centerline: vec![LngLat::new(-100.0 * m, y_m * m), LngLat::new(100.0 * m, y_m * m)], classification: Some("local".into()), row_width_m: Some(4.0) }
    }

    #[test]
    fn no_windows_is_no_view() {
        assert!(matches!(P192WindowsOverlookingLife.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_window_facing_a_nearby_street_with_no_neighbor_overlooks_life() {
        let n = nbhd(vec![building_with_window("B1", 0.0, 0.0)], vec![street_at(-10.0)]);
        let out = P192WindowsOverlookingLife.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_window_blocked_by_a_close_neighbor_is_flagged() {
        // B2 sits close to B1's window-side wall -- inside the blocked threshold --
        // while a nearby street keeps "has_life" true for both, isolating the block.
        let n = nbhd(vec![building_with_window("B1", 0.0, 0.0), building_with_window("B2", 0.0, -9.0)], vec![street_at(-10.0)]);
        let out = P192WindowsOverlookingLife.evaluate(&n);
        match out {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!(value < 1.0, "got {value}");
                assert!(!contributing_features.is_empty());
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
