//! P159 Light on Two Sides of Every Room — does the generated massing
//! actually deliver Alexander's own criterion, or just P107's weaker
//! single-wall daylight-depth guarantee?
//!
//! From Alexander, *A Pattern Language*, Pattern 159:
//! > People will always gravitate to those rooms which have light on two
//! > sides of the room, and leave the rooms which are lit only from one
//! > side unused and empty... Locate each room so that it has outdoor
//! > space on at least two sides, and then place windows in these outdoor
//! > walls so that natural light falls into every room from more than one
//! > direction.
//!
//! Confirmed (via `patternlanguage.com/apl/aplsample`, not the copyrighted
//! full-text PDF -- see repo README) that P159 explicitly names itself as
//! completing **P107 Wings of Light** (already a real operator in
//! `street-smarts-patterns`) and calls for **P221 Natural Doors and
//! Windows** (also now a real operator) among others.
//!
//! # Why this is an Opinion, not another operator
//! P159 is a criterion Alexander states about the result ("do rooms
//! actually get light from two sides"), not a shape rule to apply --
//! nothing about it tells you HOW to build a footprint, only how to judge
//! one after the fact. That's exactly this crate's job (alongside
//! `p106_positive_outdoor_space` and `p21_four_story_limit`), not
//! `street-smarts-patterns`'s.
//!
//! # What this checks
//! P107's daylight-depth guarantee (every interior point within
//! `max_wing_width_m` of AN exterior wall) is a real but WEAKER claim than
//! P159's two-sided one -- a mid-wall bay in a solid, non-courtyard
//! building satisfies P107 without ever getting light from two sides. This
//! opinion only credits a window-bearing bay as satisfying P159 when:
//! - the building is a **courtyard-ring** building (P107's
//!   `p107_courtyard_v01` typology) AND it has window bays on BOTH its
//!   outer and inner (courtyard) walls -- the whole building achieves two
//!   different daylight orientations by its very form, credited across
//!   every one of its window bays; or
//! - the building is **solid** and the window bay sits near a corner
//!   (within 15% of its wall segment's own length from either endpoint)
//!   where the ADJOINING wall segment also has a window -- a real corner
//!   room, lit from two walls that actually meet.
//!
//! `value` = the width-weighted fraction of window-bearing wall length,
//! across the neighborhood, that qualifies either way.
//!
//! No fixed-meters "near a corner" threshold: this crate is deliberately
//! decoupled from `street-smarts-patterns`'s local-projection math (see
//! `p106_positive_outdoor_space`'s own module doc for the same principle),
//! so "near a corner" is a dimensionless fraction of that wall segment's
//! own length, not a meters distance.

use std::collections::{BTreeMap, HashSet};
use street_smarts_core::components::BuildingTypology;
use street_smarts_core::nir::{Building, Neighborhood, Opening, OpeningKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

/// Within this fraction of a wall segment's own length from either
/// endpoint, a window counts as "near that corner."
const CORNER_FRAC: f64 = 0.15;

pub struct P159LightOnTwoSides;

/// The real (non-degenerate) edge count of a closed ring: `outer` follows
/// this codebase's convention of `first == last`, so the real edge count is
/// one less than the point count -- matters for the corner-adjacency
/// wraparound (`ring_index` values from `p221_natural_doors_and_windows`
/// only ever span `0..edge_count`, since its own zero-length closing "edge"
/// is filtered out there by `min_wall_segment_m` before any opening is
/// placed on it).
fn real_edge_count(outer: &[street_smarts_core::geometry::LngLat]) -> usize {
    if outer.len() >= 2 && outer.first().map(|a| (a.lng, a.lat)) == outer.last().map(|a| (a.lng, a.lat)) {
        outer.len() - 1
    } else {
        outer.len()
    }
}

/// Windows deduplicated by bay identity (same wall edge + same position),
/// keeping one per bay regardless of how many floors repeat it -- corner
/// adjacency and courtyard qualification are properties of the BAY, not
/// the floor.
fn unique_window_bays(b: &Building) -> Vec<&Opening> {
    let mut seen: HashSet<(usize, bool, i64)> = HashSet::new();
    b.openings
        .iter()
        .filter(|o| o.kind == OpeningKind::Window)
        .filter(|o| seen.insert((o.ring_index, o.on_hole, (o.t * 1000.0).round() as i64)))
        .collect()
}

impl Opinion for P159LightOnTwoSides {
    fn name(&self) -> &'static str {
        "p159_light_on_two_sides"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p159".into(),
            display: "Alexander et al., A Pattern Language, Pattern 159 (Light on Two Sides of Every Room)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl159/apl159.htm".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let with_openings: Vec<&Building> = n.buildings.iter().filter(|b| !b.openings.is_empty()).collect();
        if with_openings.is_empty() {
            return OpinionOutput::NoView {
                reason: "No building has window/door data yet -- run p221_natural_doors_and_windows first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut total_width = 0.0;
        let mut qualifying_width = 0.0;
        let mut solid_total = 0.0;
        let mut solid_qualifying = 0.0;
        let mut courtyard_total = 0.0;
        let mut courtyard_qualifying = 0.0;
        let mut n_solid = 0;
        let mut n_courtyard = 0;
        let mut low_light_buildings: Vec<String> = Vec::new();

        for b in &with_openings {
            let is_courtyard = BuildingTypology::label_is_courtyard(b.typology.as_deref());
            let windows = unique_window_bays(b);
            if windows.is_empty() {
                continue;
            }

            if is_courtyard {
                n_courtyard += 1;
                let has_outer = windows.iter().any(|o| !o.on_hole);
                let has_inner = windows.iter().any(|o| o.on_hole);
                let qualifies = has_outer && has_inner;
                let building_total: f64 = windows.iter().map(|w| w.width_m).sum();
                let building_qualifying = if qualifies { building_total } else { 0.0 };
                total_width += building_total;
                qualifying_width += building_qualifying;
                courtyard_total += building_total;
                courtyard_qualifying += building_qualifying;
                if !qualifies {
                    low_light_buildings.push(b.id.clone());
                }
            } else {
                n_solid += 1;
                let n_edges = real_edge_count(&b.polygon.outer);
                let edges_with_window: HashSet<usize> = windows.iter().map(|o| o.ring_index).collect();
                let mut building_total = 0.0;
                let mut building_qualifying = 0.0;
                for w in &windows {
                    building_total += w.width_m;
                    if n_edges == 0 {
                        continue;
                    }
                    let near_start = w.t < CORNER_FRAC;
                    let near_end = w.t > 1.0 - CORNER_FRAC;
                    let mut qualifies = false;
                    if near_start {
                        let prev = (w.ring_index + n_edges - 1) % n_edges;
                        if edges_with_window.contains(&prev) {
                            qualifies = true;
                        }
                    }
                    if near_end {
                        let next = (w.ring_index + 1) % n_edges;
                        if edges_with_window.contains(&next) {
                            qualifies = true;
                        }
                    }
                    if qualifies {
                        building_qualifying += w.width_m;
                    }
                }
                total_width += building_total;
                qualifying_width += building_qualifying;
                solid_total += building_total;
                solid_qualifying += building_qualifying;
                if building_total > 0.0 && building_qualifying / building_total < 0.3 {
                    low_light_buildings.push(b.id.clone());
                }
            }
        }

        if total_width <= 0.0 {
            return OpinionOutput::NoView {
                reason: "Buildings have opening data but no window openings to evaluate (doors only).".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let value = qualifying_width / total_width;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        if solid_total > 0.0 {
            sub_scores.insert("solid_fraction".into(), solid_qualifying / solid_total);
        }
        if courtyard_total > 0.0 {
            sub_scores.insert("courtyard_fraction".into(), courtyard_qualifying / courtyard_total);
        }

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_buildings_evaluated".into(), with_openings.len().to_string());
        details.insert("n_solid".into(), n_solid.to_string());
        details.insert("n_courtyard".into(), n_courtyard.to_string());
        details.insert("total_window_width_m".into(), format!("{:.0}", total_width));
        details.insert("qualifying_window_width_m".into(), format!("{:.0}", qualifying_width));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} building(s) with window data ({} solid, {} courtyard). {:.0}% of window-bearing \
                 wall length qualifies for two-sided light (corner bays on solid buildings, or \
                 outer+inner window bays on courtyard buildings).",
                with_openings.len(), n_solid, n_courtyard, value * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "This is a bay-level geometric proxy, not a verified per-room check -- this \
                 pipeline has no real interior room layout anywhere, so 'two sides' is judged by \
                 wall-corner adjacency (solid buildings) or outer+inner wall presence (courtyard \
                 buildings), not an actual room's own walls.".into(),
                "P107's daylight-depth guarantee (every point within max_wing_width_m of AN \
                 exterior wall) is real but weaker than P159's two-sided claim -- a solid \
                 building can satisfy P107 everywhere while most of its window bays (every \
                 non-corner one) fail this opinion's stricter check. That gap is the point of \
                 having both.".into(),
                "'Near a corner' is a dimensionless fraction (15%) of that wall segment's own \
                 length, not a fixed meters threshold -- this crate deliberately doesn't depend \
                 on street-smarts-patterns' local-projection math (see \
                 p106_positive_outdoor_space's own module doc for the same principle).".into(),
                "Evaluated once per window bay (deduplicated across floors), since corner \
                 adjacency and courtyard qualification don't depend on which floor a window is \
                 on -- doesn't separately reward or penalize a building for its floor count."
                    .into(),
            ],
            contributing_features: low_light_buildings,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon, PolygonPart};
    use street_smarts_core::nir::{NeighborhoodMeta, OpeningKind};

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.001, 0.001],
            parcels: vec![],
            buildings,
            streets: vec![],
            open_space: vec![],
            boundaries: vec![],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P159 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn square_ring(side_deg: f64) -> Vec<LngLat> {
        vec![
            LngLat::new(0.0, 0.0),
            LngLat::new(side_deg, 0.0),
            LngLat::new(side_deg, side_deg),
            LngLat::new(0.0, side_deg),
            LngLat::new(0.0, 0.0),
        ]
    }

    fn win(ring_index: usize, on_hole: bool, t: f64) -> Opening {
        Opening {
            kind: OpeningKind::Window,
            ring_index,
            on_hole,
            t,
            width_m: 1.5,
            sill_height_m: 1.0,
            head_height_m: 2.0,
            floor: 0,
        }
    }

    #[test]
    fn no_openings_anywhere_is_no_view() {
        let n = nbhd(vec![]);
        let out = P159LightOnTwoSides.evaluate(&n);
        assert!(matches!(out, OpinionOutput::NoView { .. }));
    }

    #[test]
    fn solid_building_corner_windows_qualify_midwall_ones_do_not() {
        // 4-edge square: edges 0(bottom),1(right),2(top),3(left).
        // A window near the END of edge 0 (t=0.9) is near the corner shared
        // with edge 1 -- qualifies if edge 1 also has a window.
        // A window at the MIDDLE of edge 2 (t=0.5) is nowhere near a corner
        // -- never qualifies.
        let b = Building {
            id: "SOLID_1".into(),
            polygon: Polygon::from_ring(square_ring(0.001)),
            height_m: Some(7.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(2),
            interior_cells: vec![],
            openings: vec![
                win(0, false, 0.9), // corner bay, edge 0 near its end
                win(1, false, 0.1), // corner bay, edge 1 near its start (same corner)
                win(2, false, 0.5), // mid-wall bay, no adjacent window
            ],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], };
        let n = nbhd(vec![b]);
        let out = P159LightOnTwoSides.evaluate(&n);
        match out {
            OpinionOutput::Value { value, sub_scores, contributing_features, .. } => {
                // 2 of 3 windows (equal width) qualify -> ~0.667
                assert!((value - 0.6667).abs() < 0.01, "expected ~0.667, got {value}");
                assert!(sub_scores.contains_key("solid_fraction"));
                assert!(
                    !contributing_features.contains(&"SOLID_1".to_string()),
                    "0.667 qualifying is above the 0.3 low-light flagging threshold, should not be flagged"
                );
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn courtyard_building_with_both_walls_windowed_fully_qualifies() {
        let b = Building {
            id: "CY_1".into(),
            polygon: Polygon::from_parts(vec![PolygonPart {
                outer: square_ring(0.001),
                holes: vec![square_ring(0.0004)],
            }]),
            height_m: Some(7.0),
            typology: Some("p107_courtyard_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(2),
            interior_cells: vec![],
            openings: vec![win(0, false, 0.5), win(0, true, 0.5)],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], };
        let n = nbhd(vec![b]);
        let out = P159LightOnTwoSides.evaluate(&n);
        match out {
            OpinionOutput::Value { value, sub_scores, .. } => {
                assert!((value - 1.0).abs() < 1e-6, "both outer+inner windowed -> full credit, got {value}");
                assert!((sub_scores["courtyard_fraction"] - 1.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn courtyard_building_with_only_outer_windows_does_not_qualify() {
        let b = Building {
            id: "CY_2".into(),
            polygon: Polygon::from_parts(vec![PolygonPart {
                outer: square_ring(0.001),
                holes: vec![square_ring(0.0004)],
            }]),
            height_m: Some(7.0),
            typology: Some("p107_courtyard_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(2),
            interior_cells: vec![],
            openings: vec![win(0, false, 0.5), win(1, false, 0.5)],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], };
        let n = nbhd(vec![b]);
        let out = P159LightOnTwoSides.evaluate(&n);
        match out {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!(value < 1e-6, "outer-only windows on a courtyard building shouldn't qualify, got {value}");
                assert!(contributing_features.contains(&"CY_2".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
