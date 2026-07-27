//! P165 Opening to the Street — a building's public-facing wall should be
//! substantially open (windows, doors), not a blank face broken only by
//! a single narrow entrance.
//!
//! From Alexander, *A Pattern Language*, Pattern 165, via
//! patternlanguage.cc/Patterns/Opening-to-the-Street-(165):
//! > **Problem:** The sight of action is an incentive for action. When
//! > people can see into spaces from the street their world is enlarged
//! > and made richer...
//! > **Solution:** In any public space which depends for its success on
//! > its exposure to the street, open it up, with a fully opening wall
//! > which can be thrown wide open...
//!
//! # The same real proxy the opinion already checks
//!
//! `crates/street-smarts-opinions/src/pattern/p165_opening_to_the_street.rs`
//! (the detector for this same pattern) scores buildings based on the
//! fraction of their street-facing wall covered by ground-floor openings
//! (doors and windows on the same edge). This generator re-derives that
//! same wall edge from each building's real outer-wall `Opening.ring_index`
//! where the main Door sits, then adds ground-floor Window openings on the
//! SAME edge until coverage reaches `target_coverage` (default 0.4). The
//! generator and the opinion can't silently disagree about what counts as
//! an opening or which edge is street-facing, because they use the exact
//! same real `ring_index` and floor filters.
//!
//! # What this deliberately does NOT do
//! - **No real physically-opening wall.** Same honest limitation the
//!   opinion's own caveat already states: nothing in this schema models a
//!   built opening mechanism (folding/sliding wall). This adds real static
//!   window openings, not a wall that physically opens.
//! - **Ground floor only.** Upper-floor transparency is not checked or
//!   modified -- `floor: 0` only.
//! - **Assumes door placement already identified the street-facing wall.**
//!   Reuses `p221_natural_doors_and_windows`'s own heuristic for which wall
//!   edge faces the street, via the `ring_index` where the main Door sits.
//!   Does not independently verify which wall actually faces a real street.
//! - **target_coverage (0.4) is a judgment call.** Alexander's text describes
//!   a mechanism, not a percentage -- this threshold is our own interpretation.

use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::{Building, Neighborhood, OpeningKind, Opening};
use street_smarts_core::opinion::SourceCitation;

/// Matches the real opinion's own `TARGET_COVERAGE` exactly -- see
/// this module's own doc for why that agreement matters.
const DEFAULT_TARGET_COVERAGE: f64 = 0.4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P165Params {
    /// Target fraction of street-facing wall to be covered by ground-floor
    /// openings (door + windows on the same edge).
    pub target_coverage: f64,
    /// Minimum window width in meters.
    pub min_window_width_m: f64,
    /// Maximum window width in meters.
    pub max_window_width_m: f64,
    /// Maximum number of windows to add per building.
    pub max_windows: u32,
}

impl Parameters for P165Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "target_coverage",
                "Target fraction of street-facing wall covered by ground-floor openings.",
                0.1,
                0.9,
                DEFAULT_TARGET_COVERAGE,
            ),
            ParamSpec::float("min_window_width_m", "Minimum window width in meters.", 0.3, 2.0, 0.9)
                .with_unit("m"),
            ParamSpec::float("max_window_width_m", "Maximum window width in meters.", 1.0, 6.0, 3.0)
                .with_unit("m"),
            ParamSpec::integer("max_windows", "Maximum number of windows to add per building.", 1.0, 12.0, 6.0),
        ]
    }
    fn defaults() -> Self {
        Self {
            target_coverage: DEFAULT_TARGET_COVERAGE,
            min_window_width_m: 0.9,
            max_window_width_m: 3.0,
            max_windows: 6,
        }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![
            self.target_coverage,
            self.min_window_width_m,
            self.max_window_width_m,
            self.max_windows as f64,
        ]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) {
            p.target_coverage = s.clamp(*x);
        }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) {
            p.min_window_width_m = s.clamp(*x);
        }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) {
            p.max_window_width_m = s.clamp(*x);
        }
        if let (Some(s), Some(x)) = (schema.get(3), v.get(3)) {
            p.max_windows = s.clamp(*x) as u32;
        }
        p
    }
}

pub struct P165OpeningToTheStreet;

impl PatternOperator for P165OpeningToTheStreet {
    type Params = P165Params;

    fn name(&self) -> &'static str {
        "p165_opening_to_the_street"
    }
    fn description(&self) -> &'static str {
        "Adds ground-floor window openings to building facades to reach target street-facing wall coverage."
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p165".into(),
            display: "Alexander et al., A Pattern Language, Pattern 165 (Opening to the Street)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Opening-to-the-Street-(165)".into()),
        }
    }

    /// `parcel_id` must be `"*"` -- processes all buildings in one pass.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p165_opening_to_the_street only supports parcel_id \"*\" -- it processes all buildings in one pass.".into());
        }

        // Find all buildings with outer-wall doors
        let buildings_with_doors: Vec<_> = nbhd.buildings.iter()
            .filter(|b| b.openings.iter().any(|o| o.kind == OpeningKind::Door && !o.on_hole))
            .collect();

        if buildings_with_doors.is_empty() {
            return Err(
                "p165_opening_to_the_street: no building has a real outer-wall door opening yet -- run p221_natural_doors_and_windows first.".into(),
            );
        }

        let mut new_buildings: Vec<Building> = Vec::new();
        let mut replaced_building_ids: Vec<String> = Vec::new();
        let mut n_buildings_improved = 0usize;
        let mut n_windows_added = 0usize;
        let mut buildings_already_meeting_target = 0usize;
        let mut shortfall_count = 0usize;

        for b in &nbhd.buildings {
            // Skip buildings without an outer-wall door
            let door = match b.openings.iter().find(|o| o.kind == OpeningKind::Door && !o.on_hole) {
                Some(d) => d,
                None => continue,
            };

            let ring = &b.polygon.outer;
            // Guard against invalid ring_index
            if door.ring_index + 1 >= ring.len() {
                continue;
            }

            let wall_len = haversine_m(&ring[door.ring_index], &ring[door.ring_index + 1]);
            if wall_len <= 0.0 {
                continue;
            }

            // Calculate current opening coverage
            let opening_width: f64 = b.openings.iter()
                .filter(|o| !o.on_hole && o.ring_index == door.ring_index && o.floor == 0)
                .map(|o| o.width_m)
                .sum();
            let current_coverage = (opening_width / wall_len).min(1.0);

            // If already meets target, skip
            if current_coverage >= params.target_coverage {
                buildings_already_meeting_target += 1;
                continue;
            }

            // Calculate how much width we need to add
            let target_width = params.target_coverage * wall_len;
            let deficit_m = target_width - opening_width;

            // Compute number of windows and size
            let n_windows_needed = ((deficit_m / params.max_window_width_m).ceil() as u32)
                .min(params.max_windows)
                .max(1);
            let window_width = (deficit_m / n_windows_needed as f64)
                .max(params.min_window_width_m)
                .min(params.max_window_width_m);

            // Try to place windows
            let mut nb = b.clone();
            let door_half_width_frac = (door.width_m / 2.0) / wall_len;
            let door_start = door.t - door_half_width_frac;
            let door_end = door.t + door_half_width_frac;

            let window_width_frac = window_width / wall_len;

            // Collect existing windows on this edge so we don't overlap them either
            let existing_window_positions: Vec<(f64, f64)> = b.openings.iter()
                .filter(|o| o.kind == OpeningKind::Window && !o.on_hole && o.ring_index == door.ring_index && o.floor == 0)
                .map(|w| {
                    let w_half_frac = (w.width_m / 2.0) / wall_len;
                    (w.t - w_half_frac, w.t + w_half_frac)
                })
                .collect();

            // Create candidate slots evenly distributed along the wall
            let n_candidates = (params.max_windows as usize) * 2;
            let mut placed_windows = 0;
            let mut placed_spans: Vec<(f64, f64)> = Vec::new();

            for i in 0..n_candidates {
                if placed_windows >= n_windows_needed {
                    break;
                }
                let t = (i as f64 + 0.5) / n_candidates as f64;
                let w_start = t - window_width_frac / 2.0;
                let w_end = t + window_width_frac / 2.0;

                // Check bounds
                if w_start < 0.0 || w_end > 1.0 {
                    continue;
                }

                // Check overlap with door
                if w_end > door_start && w_start < door_end {
                    continue;
                }

                // Check overlap with existing windows
                if existing_window_positions.iter().any(|(es, ee)| w_end > *es && w_start < *ee) {
                    continue;
                }

                // Check overlap with already-placed windows from this run
                if placed_spans.iter().any(|(ps, pe)| w_end > *ps && w_start < *pe) {
                    continue;
                }

                // Place the window
                nb.openings.push(Opening {
                    kind: OpeningKind::Window,
                    ring_index: door.ring_index,
                    on_hole: false,
                    t,
                    width_m: window_width,
                    sill_height_m: 0.9,
                    head_height_m: 2.1,
                    floor: 0,
                });
                placed_spans.push((w_start, w_end));
                placed_windows += 1;
                n_windows_added += 1;
            }

            // Check if we placed fewer windows than needed
            if placed_windows < n_windows_needed {
                shortfall_count += 1;
            }

            n_buildings_improved += 1;
            new_buildings.push(nb);
            replaced_building_ids.push(b.id.clone());
        }

        if new_buildings.is_empty() {
            return Err(format!(
                "p165_opening_to_the_street: {} buildings with outer-wall doors already meet the {:.0}% target coverage -- nothing to do.",
                buildings_already_meeting_target,
                params.target_coverage * 100.0
            ));
        }

        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!(
                "Added {} window opening(s) to {} building(s) to reach {:.0}% street-facing wall coverage.",
                n_windows_added,
                n_buildings_improved,
                params.target_coverage * 100.0
            ),
            steps: vec![
                format!(
                    "{} building(s) with outer-wall doors had coverage below the {:.0}% target; {} window(s) added across {} building(s).",
                    n_buildings_improved,
                    params.target_coverage * 100.0,
                    n_windows_added,
                    n_buildings_improved
                ),
                if shortfall_count > 0 {
                    format!(
                        "{} building(s) could not fit enough non-overlapping windows to reach the target on available wall space.",
                        shortfall_count
                    )
                } else {
                    "All buildings reached the target coverage.".into()
                },
            ],
            caveats: vec![
                "Cannot check Alexander's literal requirement at all -- a wall that physically \
                 opens (folding/sliding), or activity straddling the pedestrian path. This is a \
                 static-transparency proxy (opening width / wall length) only.".into(),
                "target_coverage (0.4) is a judgment call, not a number Alexander's text gives -- \
                 he describes a mechanism, not a percentage.".into(),
                "Ground floor only (Opening.floor == 0) -- doesn't check or modify upper-floor \
                 transparency, which this pattern doesn't distinguish but a real streetscape \
                 assessment would.".into(),
                "Assumes the wall edge carrying the main door is the street-facing one, reusing \
                 p221_natural_doors_and_windows's own door-placement heuristic rather than \
                 independently verifying which wall actually faces a real street.".into(),
                format!(
                    "{} building(s) with outer-wall doors already met the target coverage and were left \
                     unchanged.",
                    buildings_already_meeting_target
                ).into(),
            ],
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels: vec![],
            new_open_space: vec![],
            new_buildings,
            new_streets: vec![],
            new_activity_nodes: vec![],
            new_boundaries: vec![],
            new_fields: vec![],
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: vec![],
            replaced_building_ids,
            entity_provenance: Default::default(),
            trace,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{NeighborhoodMeta, Street};

    fn m() -> f64 {
        1.0 / 111_320.0
    }

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![],
            buildings,
            streets: vec![] as Vec<Street>,
            open_space: vec![],
            boundaries: vec![],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P165 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    /// A 20m x 10m rectangle -- outer ring edge 0 is the 20m-long bottom wall.
    fn rect_building(id: &str, openings: Vec<Opening>) -> Building {
        let mm = m();
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0),
                LngLat::new(20.0 * mm, 0.0),
                LngLat::new(20.0 * mm, 10.0 * mm),
                LngLat::new(0.0, 10.0 * mm),
                LngLat::new(0.0, 0.0),
            ]),
            height_m: Some(9.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(3),
            openings,
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
            canopies: vec![],
            roof_segments: vec![],
            wall_niches: vec![],
        }
    }

    fn door_opening(ring_index: usize, width_m: f64) -> Opening {
        Opening {
            kind: OpeningKind::Door,
            ring_index,
            on_hole: false,
            t: 0.5,
            width_m,
            sill_height_m: 0.0,
            head_height_m: 2.1,
            floor: 0,
        }
    }

    fn window_opening(ring_index: usize, t: f64, width_m: f64) -> Opening {
        Opening {
            kind: OpeningKind::Window,
            ring_index,
            on_hole: false,
            t,
            width_m,
            sill_height_m: 0.9,
            head_height_m: 2.1,
            floor: 0,
        }
    }

    #[test]
    fn no_buildings_with_doors_is_an_error() {
        let n = nbhd(vec![rect_building("B1", vec![])]);
        let err = P165OpeningToTheStreet.apply(&n, "*", &P165Params::defaults(), 0).unwrap_err();
        assert!(
            err.contains("p221"),
            "expected error mentioning p221 prerequisite, got: {err}"
        );
    }

    #[test]
    fn parcel_id_must_be_wildcard() {
        let n = nbhd(vec![rect_building("B1", vec![door_opening(0, 0.9)])]);
        let err = P165OpeningToTheStreet.apply(&n, "BLOCK_0", &P165Params::defaults(), 0).unwrap_err();
        assert!(err.contains("\"*\""), "expected the real target-scope error, got: {err}");
    }

    #[test]
    fn a_narrow_door_below_target_gets_windows_added() {
        // 0.9m door on a 20m wall -> 4.5% coverage, well under 40% target.
        // Need to add windows to reach 8m coverage (40% of 20m).
        // Target deficit: 8m - 0.9m = 7.1m
        let n = nbhd(vec![rect_building("B1", vec![door_opening(0, 0.9)])]);
        let sub = P165OpeningToTheStreet.apply(&n, "*", &P165Params::defaults(), 0).unwrap();

        assert_eq!(sub.new_buildings.len(), 1);
        assert_eq!(sub.replaced_building_ids, vec!["B1"]);

        let b = &sub.new_buildings[0];
        // Should have the original door plus new windows
        assert!(b.openings.len() > 1, "should have added windows");

        // Count windows added
        let n_windows = b.openings.iter().filter(|o| o.kind == OpeningKind::Window).count();
        assert!(n_windows > 0, "should have added at least one window");

        // All new openings should be on ring_index 0 (same as door) and floor 0
        for o in &b.openings {
            if o.kind == OpeningKind::Window {
                assert_eq!(o.ring_index, 0, "window should be on same edge as door");
                assert_eq!(o.floor, 0, "window should be on ground floor");
                assert!(!o.on_hole, "window should be on outer wall");
                assert!(o.width_m >= 0.9 && o.width_m <= 3.0, "window width should respect min/max");
            }
        }

        // Check that total opening width is substantially increased
        let total_opening_width: f64 = b.openings.iter()
            .filter(|o| !o.on_hole && o.ring_index == 0 && o.floor == 0)
            .map(|o| o.width_m)
            .sum();
        // Should be at least 7m (vs original 0.9m door)
        assert!(total_opening_width > 5.0, "should have added significant window coverage, got {}", total_opening_width);
    }

    #[test]
    fn all_buildings_meeting_target_is_an_error() {
        // To meet 40% of 20m wall, need 8m opening width.
        // Use door (0.9m) + four windows (2.0m each) = 8.9m = 44.5% coverage.
        let n = nbhd(vec![rect_building(
            "B1",
            vec![
                door_opening(0, 0.9),
                window_opening(0, 0.15, 2.0),
                window_opening(0, 0.40, 2.0),
                window_opening(0, 0.65, 2.0),
                window_opening(0, 0.90, 2.0),
            ],
        )]);
        let err = P165OpeningToTheStreet.apply(&n, "*", &P165Params::defaults(), 0).unwrap_err();
        assert!(err.contains("nothing to do"), "expected 'nothing to do' error, got: {err}");
    }

    #[test]
    fn nothing_to_do_error_is_real_not_silent_empty() {
        // Same as above: building already meets target
        let n = nbhd(vec![
            rect_building("B1", vec![
                door_opening(0, 0.9),
                window_opening(0, 0.15, 2.0),
                window_opening(0, 0.40, 2.0),
                window_opening(0, 0.65, 2.0),
                window_opening(0, 0.90, 2.0),
            ]),
        ]);
        let result = P165OpeningToTheStreet.apply(&n, "*", &P165Params::defaults(), 0);
        assert!(result.is_err(), "should error, not return empty subdivision");
    }

    #[test]
    fn params_roundtrip() {
        let p = P165Params {
            target_coverage: 0.5,
            min_window_width_m: 0.8,
            max_window_width_m: 2.5,
            max_windows: 8,
        };
        let v = p.as_vector();
        let back = P165Params::from_vector(&v);
        assert!((back.target_coverage - 0.5).abs() < 1e-9);
        assert!((back.min_window_width_m - 0.8).abs() < 1e-9);
        assert!((back.max_window_width_m - 2.5).abs() < 1e-9);
        assert_eq!(back.max_windows, 8);
    }

    #[test]
    fn windows_dont_overlap_each_other() {
        let n = nbhd(vec![rect_building("B1", vec![door_opening(0, 0.9)])]);
        let sub = P165OpeningToTheStreet.apply(&n, "*", &P165Params::defaults(), 0).unwrap();
        let b = &sub.new_buildings[0];

        let mm = m();
        let wall_len = 20.0 * mm * (111_320.0);

        // Collect windows and their spans
        let windows: Vec<_> = b.openings.iter()
            .filter(|o| o.kind == OpeningKind::Window && o.ring_index == 0 && o.floor == 0)
            .collect();

        for i in 0..windows.len() {
            for j in (i + 1)..windows.len() {
                let w1 = windows[i];
                let w2 = windows[j];

                let w1_half_frac = (w1.width_m / 2.0) / wall_len;
                let w1_start = w1.t - w1_half_frac;
                let w1_end = w1.t + w1_half_frac;

                let w2_half_frac = (w2.width_m / 2.0) / wall_len;
                let w2_start = w2.t - w2_half_frac;
                let w2_end = w2.t + w2_half_frac;

                // Check no overlap
                assert!(
                    w1_end <= w2_start || w2_end <= w1_start,
                    "windows {i} and {j} overlap: [{w1_start}, {w1_end}] vs [{w2_start}, {w2_end}]"
                );
            }
        }
    }

    #[test]
    fn windows_dont_overlap_the_door() {
        let n = nbhd(vec![rect_building("B1", vec![door_opening(0, 0.9)])]);
        let sub = P165OpeningToTheStreet.apply(&n, "*", &P165Params::defaults(), 0).unwrap();
        let b = &sub.new_buildings[0];

        let mm = m();
        let wall_len = 20.0 * mm * (111_320.0);

        // Find the door
        let door = b.openings.iter().find(|o| o.kind == OpeningKind::Door && o.ring_index == 0).unwrap();
        let door_half_frac = (door.width_m / 2.0) / wall_len;
        let door_start = door.t - door_half_frac;
        let door_end = door.t + door_half_frac;

        // Check windows don't overlap door
        for w in b.openings.iter().filter(|o| o.kind == OpeningKind::Window && o.ring_index == 0) {
            let w_half_frac = (w.width_m / 2.0) / wall_len;
            let w_start = w.t - w_half_frac;
            let w_end = w.t + w_half_frac;

            assert!(
                w_end <= door_start || w_start >= door_end,
                "window overlaps door: window [{w_start}, {w_end}] vs door [{door_start}, {door_end}]"
            );
        }
    }
}
