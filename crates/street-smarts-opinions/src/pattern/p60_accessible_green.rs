//! P60 Accessible Green — a real, generously-sized green space within a
//! three-minute walk of every building, not a token patch.
//!
//! From Alexander, *A Pattern Language*, Pattern 60, via
//! patternlanguage.cc/Patterns/Accessible-Green-(60):
//! > **Problem:** People need green open places to go to; when they are
//! > close they use them. But if the greens are more than three minutes
//! > away, the distance overwhelms the need.
//! > **Solution:** Build one open public green within three minutes'
//! > walk -- about 750 feet -- of every house and workplace... Make the
//! > greens at least 150 feet across, and at least 60,000 square feet in
//! > area.
//!
//! # A likely real gap, checked honestly either way
//!
//! Alexander's numbers here (150ft/45.7m minimum width, 60,000 sq ft/
//! ~5,574 m² minimum area) are a much larger scale than
//! `p61_small_public_squares`'s own target (Alexander's P61 "60 feet"
//! max_dimension_m default for THAT pattern, a deliberately small,
//! intimate square -- see its own module doc). A green qualifying for
//! P60 needs to be roughly two orders of magnitude larger in area than a
//! typical P61 square. This opinion doesn't assume that gap exists --
//! it checks real open-space geometry against Alexander's real numbers,
//! whatever the current pipeline actually produces.
//!
//! # Two sub-scores
//!
//! - `qualifying_green_size`: fraction of real, non-`Undecided` open-space
//!   features meeting BOTH area >= 5,574 m² AND minimum bounding-box
//!   dimension >= 45.7m (a real-shape proxy for "at least 150 feet
//!   across" -- a long thin sliver of the right area alone wouldn't
//!   satisfy Alexander's intent).
//! - `walking_distance`: fraction of buildings within 229m (~750 feet,
//!   straight-line) of at least one qualifying green.
//!
//! `value = 0.5 * qualifying_green_size + 0.5 * walking_distance`.
//! `walking_distance` needs at least one qualifying green to mean
//! anything; falls back to `qualifying_green_size` alone (which will be
//! 0.0 if none qualify) otherwise.

use std::collections::BTreeMap;
use street_smarts_core::geometry::{haversine_m, LngLat};
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P60AccessibleGreen;

const WALK_THRESHOLD_M: f64 = 229.0; // ~750 feet
const MIN_AREA_M2: f64 = 5_574.0; // 60,000 sq ft
const MIN_WIDTH_M: f64 = 45.7; // 150 feet

/// Minimum bounding-box dimension for a WGS84 ring, in meters -- same
/// local-projection technique `p106_positive_outdoor_space`'s own `hull`
/// module uses, scoped to exactly this.
fn min_bbox_dimension_m(ring: &[LngLat]) -> f64 {
    if ring.is_empty() {
        return 0.0;
    }
    let lat0 = ring.iter().map(|p| p.lat).sum::<f64>() / ring.len() as f64;
    let mlat = (lat0.to_radians()).cos();
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

impl Opinion for P60AccessibleGreen {
    fn name(&self) -> &'static str {
        "p60_accessible_green"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p60".into(),
            display: "Alexander et al., A Pattern Language, Pattern 60 (Accessible Green)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Accessible-Green-(60)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let candidates: Vec<_> = n.open_space.iter().filter(|o| o.kind != OpenSpaceKind::Undecided).collect();
        if candidates.is_empty() {
            return OpinionOutput::NoView {
                reason: "No resolved (non-Undecided) open space in this neighborhood.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut qualifying: Vec<LngLat> = Vec::new();
        let mut too_small: Vec<String> = Vec::new();
        for o in &candidates {
            let area = o.polygon.area_m2();
            let width = min_bbox_dimension_m(&o.polygon.outer);
            if area >= MIN_AREA_M2 && width >= MIN_WIDTH_M {
                qualifying.push(o.polygon.centroid());
            } else {
                too_small.push(o.id.clone());
            }
        }
        let qualifying_green_size = qualifying.len() as f64 / candidates.len() as f64;

        let walking_distance = if qualifying.is_empty() || n.buildings.is_empty() {
            None
        } else {
            let n_within = n.buildings.iter()
                .filter(|b| {
                    let c = b.polygon.centroid();
                    qualifying.iter().any(|g| haversine_m(&c, g) <= WALK_THRESHOLD_M)
                })
                .count();
            Some(n_within as f64 / n.buildings.len() as f64)
        };

        let value = match walking_distance {
            Some(w) => 0.5 * qualifying_green_size + 0.5 * w,
            None => qualifying_green_size,
        };

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("qualifying_green_size".into(), qualifying_green_size);
        if let Some(w) = walking_distance {
            sub_scores.insert("walking_distance".into(), w);
        }

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_open_space_candidates".into(), candidates.len().to_string());
        details.insert("n_qualifying_greens".into(), qualifying.len().to_string());
        details.insert("min_area_m2".into(), format!("{MIN_AREA_M2:.0}"));
        details.insert("min_width_m".into(), format!("{MIN_WIDTH_M:.1}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} open-space feature(s); {} qualify as a real Accessible Green (>= {:.0}m², >= \
                 {:.0}m across).{}",
                candidates.len(), qualifying.len(), MIN_AREA_M2, MIN_WIDTH_M,
                walking_distance.map(|w| format!(" {:.0}% of buildings are within {:.0}m ({:.0}ft) of one.", w * 100.0, WALK_THRESHOLD_M, WALK_THRESHOLD_M * 3.28)).unwrap_or_default(),
            ),
            sub_scores,
            details,
            caveats: vec![
                "walking_distance is straight-line distance, not a real three-minute walking-\
                 network time -- Alexander's own '750 feet' is itself a walking-time conversion, \
                 so this compounds one estimate on top of another.".into(),
                "min_bbox_dimension is a coarse width proxy (local bounding-box, not the polygon's \
                 own narrowest real cross-section) -- an L-shaped green could pass this check on \
                 its bounding box while having a much narrower real pinch point.".into(),
                "Doesn't check the '1,500-foot uniform scattering' interval between greens \
                 separately from walking distance -- treated as implied by walking_distance rather \
                 than scored on its own.".into(),
            ],
            contributing_features: too_small,
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
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.02, 0.02],
            parcels: vec![], buildings, streets: vec![], open_space,
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P60 unit fixture".into(),
            },
        }
    }

    fn square_open_space(id: &str, cx_m: f64, cy_m: f64, side_m: f64, kind: OpenSpaceKind) -> OpenSpace {
        let m = 1.0 / 111_320.0;
        let half = side_m / 2.0;
        OpenSpace {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new((cx_m - half) * m, (cy_m - half) * m), LngLat::new((cx_m + half) * m, (cy_m - half) * m),
                LngLat::new((cx_m + half) * m, (cy_m + half) * m), LngLat::new((cx_m - half) * m, (cy_m + half) * m),
                LngLat::new((cx_m - half) * m, (cy_m - half) * m),
            ]),
            kind,
        }
    }

    fn building_at(id: &str, x_m: f64, y_m: f64) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(x_m * m, y_m * m), LngLat::new((x_m + 10.0) * m, y_m * m),
                LngLat::new((x_m + 10.0) * m, (y_m + 10.0) * m), LngLat::new(x_m * m, (y_m + 10.0) * m),
                LngLat::new(x_m * m, y_m * m),
            ]),
            height_m: Some(9.0), typology: Some("p107_solid_v01".into()),
            year_built: None, parcel_id: None, floors: Some(3), openings: vec![],
            interior_cells: vec![],
        }
    }

    #[test]
    fn no_resolved_open_space_is_no_view() {
        assert!(matches!(P60AccessibleGreen.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_p61_scale_square_does_not_qualify() {
        // 18m square, matching P61's own real max_dimension_m default --
        // far below the 60,000 sq ft (5,574 m²) Alexander requires here.
        let n = nbhd(vec![square_open_space("SQ1", 0.0, 0.0, 18.0, OpenSpaceKind::Plaza)], vec![]);
        let out = P60AccessibleGreen.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, contributing_features, .. } => {
                assert!((sub_scores["qualifying_green_size"] - 0.0).abs() < 1e-6);
                assert!(contributing_features.contains(&"SQ1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_real_75m_square_park_qualifies() {
        // 75m x 75m = 5,625 m², just over the 5,574 m² threshold, width 75m > 45.7m.
        let n = nbhd(vec![square_open_space("PARK1", 0.0, 0.0, 75.0, OpenSpaceKind::Park)], vec![]);
        let out = P60AccessibleGreen.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["qualifying_green_size"] - 1.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn buildings_within_walking_distance_of_a_qualifying_green_score_high() {
        let park = square_open_space("PARK1", 0.0, 0.0, 75.0, OpenSpaceKind::Park);
        let close_building = building_at("B_CLOSE", 100.0, 0.0);
        let far_building = building_at("B_FAR", 5000.0, 0.0);
        let n = nbhd(vec![park], vec![close_building, far_building]);
        let out = P60AccessibleGreen.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["walking_distance"] - 0.5).abs() < 1e-6, "exactly one of two buildings should be within range, got {}", sub_scores["walking_distance"]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
