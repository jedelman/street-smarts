//! P164 Street Windows — buildings that run alongside real streets need
//! real windows looking out onto them, not blind walls.
//!
//! From Alexander, *A Pattern Language*, Pattern 164 (p. 769), via
//! patternlanguage.cc/Patterns/Street-Windows-(164):
//! > **Problem:** A street without windows is blind and frightening. And
//! > it is equally uncomfortable to be in a house which bounds a public
//! > street with no window at all on the street.
//! > **Solution:** Where buildings run alongside busy streets, build
//! > windows with window seats, looking out onto the street.
//!
//! # A real, checkable proxy -- same idiom as p192, narrowed to streets
//!
//! Reuses `p192_windows_overlooking_life.rs`'s own ring-interpolation
//! `opening_point` helper and ground-floor, outer-wall window filter, but
//! narrowed specifically to Local- or Pedestrian-classified streets (not
//! any resolved open space) -- matching this pattern's own "alongside
//! busy streets" claim rather than P192's broader "life" check. For each
//! building adjacent to (within `STREET_ADJACENCY_M`, 20m) a Local/
//! Pedestrian street, checks whether it has at least one real
//! ground-floor window whose exterior point sits within
//! `WINDOW_THRESHOLD_M` (15m) of that street's own centerline.

use std::collections::BTreeMap;
use street_smarts_core::geometry::{point_to_polyline_m, LngLat};
use street_smarts_core::nir::{Neighborhood, OpeningKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P164StreetWindows;

const STREET_ADJACENCY_M: f64 = 20.0;
const WINDOW_THRESHOLD_M: f64 = 15.0;

fn opening_point(ring: &[LngLat], ring_index: usize, t: f64) -> Option<LngLat> {
    if ring.len() < 2 {
        return None;
    }
    let idx = ring_index.min(ring.len().saturating_sub(2));
    let a = ring[idx];
    let c = ring[(idx + 1) % ring.len()];
    Some(LngLat::new(a.lng + (c.lng - a.lng) * t, a.lat + (c.lat - a.lat) * t))
}

fn is_walkable(classification: &Option<String>) -> bool {
    matches!(classification.as_deref(), Some("local") | Some("pedestrian"))
}

impl Opinion for P164StreetWindows {
    fn name(&self) -> &'static str {
        "p164_street_windows"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p164".into(),
            display: "Alexander et al., A Pattern Language, Pattern 164 (Street Windows)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Street-Windows-(164)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let walk_streets: Vec<_> = n.streets.iter().filter(|s| is_walkable(&s.classification)).collect();
        if walk_streets.is_empty() {
            return OpinionOutput::NoView {
                reason: "No Local- or Pedestrian-classified street in this neighborhood.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut checked = 0usize;
        let mut has_window = 0usize;
        let mut blind: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for b in &n.buildings {
            let bc = b.polygon.centroid();
            let nearest_street_m = walk_streets.iter().map(|s| point_to_polyline_m(&bc, &s.centerline)).fold(f64::MAX, f64::min);
            if nearest_street_m > STREET_ADJACENCY_M {
                continue;
            }
            checked += 1;

            let has_facing_window = b.openings.iter()
                .filter(|w| w.kind == OpeningKind::Window && !w.on_hole && w.floor == 0)
                .filter_map(|w| opening_point(&b.polygon.outer, w.ring_index, w.t))
                .any(|p| walk_streets.iter().any(|s| point_to_polyline_m(&p, &s.centerline) <= WINDOW_THRESHOLD_M));

            details.insert(format!("{}.nearest_walkable_street_m", b.id), format!("{:.1}", nearest_street_m));
            if has_facing_window {
                has_window += 1;
            } else {
                blind.push(b.id.clone());
            }
        }

        if checked == 0 {
            return OpinionOutput::NoView {
                reason: "No building sits within the street-adjacency threshold of a Local/\
                         Pedestrian street."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let value = has_window as f64 / checked as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("has_street_window_fraction".into(), value);
        details.insert("n_street_adjacent_buildings".into(), checked.to_string());
        details.insert("street_adjacency_m".into(), format!("{STREET_ADJACENCY_M:.0}"));
        details.insert("window_threshold_m".into(), format!("{WINDOW_THRESHOLD_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} building(s) adjacent to a Local/Pedestrian street; {:.0}% have a real \
                 ground-floor window within {:.0}m of that street's own centerline.",
                checked, value * 100.0, WINDOW_THRESHOLD_M
            ),
            sub_scores,
            details,
            caveats: vec![
                "'Busy' collapses to Street.classification's Local/Pedestrian values -- no traffic \
                 volume or pedestrian-count concept exists to distinguish a genuinely busy street.".into(),
                "Cannot check for a real window seat -- no furniture/interior-fixture concept exists \
                 in this schema.".into(),
                "Doesn't verify the window's own facing direction points at the street, only that its \
                 exterior point sits within the threshold distance.".into(),
            ],
            contributing_features: blind,
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
                layer_provenance: Default::default(), label: "P164 unit fixture".into(),
            },
        }
    }

    fn local_street(y_m: f64) -> Street {
        let m = 1.0 / 111_320.0;
        Street { id: "S1".into(), centerline: vec![LngLat::new(-100.0 * m, y_m * m), LngLat::new(100.0 * m, y_m * m)], classification: Some("local".into()), row_width_m: Some(5.5) }
    }

    fn building_with_window(id: &str, y_m: f64, has_window: bool) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(-3.0 * m, y_m * m), LngLat::new(3.0 * m, y_m * m),
                LngLat::new(3.0 * m, (y_m + 6.0) * m), LngLat::new(-3.0 * m, (y_m + 6.0) * m),
                LngLat::new(-3.0 * m, y_m * m),
            ]),
            height_m: Some(6.0), typology: None, year_built: None, parcel_id: None, floors: Some(1),
            openings: if has_window {
                vec![Opening { kind: OpeningKind::Window, ring_index: 0, on_hole: false, t: 0.5, width_m: 1.2, sill_height_m: 0.9, head_height_m: 2.1, floor: 0 }]
            } else {
                vec![]
            },
            interior_cells: vec![],
        }
    }

    #[test]
    fn no_walkable_streets_is_no_view() {
        assert!(matches!(P164StreetWindows.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_street_facing_window_scores_full() {
        let n = nbhd(vec![building_with_window("B1", 5.0, true)], vec![local_street(0.0)]);
        match P164StreetWindows.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_blind_building_is_flagged() {
        let n = nbhd(vec![building_with_window("B1", 5.0, false)], vec![local_street(0.0)]);
        match P164StreetWindows.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-6, "got {value}");
                assert!(contributing_features.contains(&"B1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
