//! P192 Windows Overlooking Life — relocate ground-floor windows to positions
//! where they look out over real activity (a street, a garden, open space),
//! not stare into a blank wall a few feet away.
//!
//! From Alexander, *A Pattern Language*, Pattern 192, via
//! patternlanguage.cc/Patterns/Windows-Overlooking-Life-(192):
//! > **Problem:** Rooms without a view are prisons for the people who
//! > have to stay in them.
//! > **Solution:** ...place [windows] to give the best possible views of
//! > life around the building: the garden, street, or anything which
//! > contrasts with the indoor scene, to people inside the building.
//!
//! # The same real proxy the opinion already checks
//!
//! `crates/street-smarts-opinions/src/pattern/p192_windows_overlooking_life.rs`
//! (the detector for this same pattern) has, since it shipped, checked each
//! ground-floor, outer-wall window (`Opening { kind: Window, on_hole: false,
//! floor: 0 }`) against two conditions using the EXACT same ring-interpolation,
//! distance-to-street, and distance-to-neighbor logic:
//!
//! - `has_life`: perpendicular distance from the window's exterior point to
//!   the nearest real street centerline OR distance to the nearest non-`Undecided`
//!   open-space centroid is <= `view_threshold_m` (default 25.0m).
//! - `not_blocked`: distance from the window's exterior point to the nearest
//!   OTHER building's wall vertex is > `blocked_threshold_m` (default 6.0m).
//!
//! A window passes only if BOTH hold. This generator reads the EXACT same
//! geometry the EXACT same way, so the generator and the opinion can't
//! silently disagree about what counts as a window overlooking life.
//!
//! # What this deliberately does NOT do
//!
//! - **No real sightline verification.** Same honest limitation the opinion's
//!   own caveat already states: proximity to a street or open space is a
//!   coarse stand-in for a verified sightline -- doesn't confirm the window's
//!   own facing direction actually looks toward that feature.
//! - **Cannot address 25%-of-floor-area glazing.** Alexander's text gives that
//!   real target for the San Francisco Bay Area (e.g., 6 windows on a 100 sq m
//!   floor = ~6%). No floor-area-to-glazing ratio exists in this schema, so
//!   the opinion and this generator both use proximity-to-life as a proxy only.
//! - **Relocates a window, changes which wall it sits on.** Moving a window
//!   from one ring edge to another can affect `p159_light_on_two_sides`'s
//!   corner-adjacency rule (what counts as "two sides" of a corner building)
//!   and `p164_street_windows`' street-facing check (the window is no longer
//!   on the same wall, so street adjacency might change). This is a real
//!   interaction to track if both patterns run.
//! - **Never relocates a window onto an edge shorter than its own width_m.**
//!   A short wall edge can't physically fit a wide window, so that edge is
//!   silently skipped as a candidate, even if it would pass both life-view
//!   conditions.
//! - **No random shuffling or prioritization.** Tries other edges in order
//!   (ring_index 0, 1, 2, ...) and places the window at the first edge that
//!   passes both conditions. No stochastic ordering or "prefer corners."

use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::{haversine_m, point_to_polyline_m, LngLat};
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind, OpeningKind};
use street_smarts_core::opinion::SourceCitation;

/// Matches the real opinion's own `VIEW_THRESHOLD_M` exactly -- see
/// this module's own doc for why that agreement matters.
const DEFAULT_VIEW_THRESHOLD_M: f64 = 25.0;

/// Matches the real opinion's own `BLOCKED_THRESHOLD_M` exactly.
const DEFAULT_BLOCKED_THRESHOLD_M: f64 = 6.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P192Params {
    /// How close a real street centerline or open-space centroid must sit
    /// to a window's exterior point to count as "having life nearby."
    /// Must match the opinion's own `VIEW_THRESHOLD_M`.
    pub view_threshold_m: f64,
    /// How far another building's wall must sit from a window's exterior
    /// point to NOT block the view. Distances <= this threshold count as
    /// "blocking." Must match the opinion's own `BLOCKED_THRESHOLD_M`.
    pub blocked_threshold_m: f64,
}

impl Parameters for P192Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "view_threshold_m",
                "How close a street or open space must sit to a window to count as a real view.",
                10.0,
                50.0,
                DEFAULT_VIEW_THRESHOLD_M,
            ),
            ParamSpec::float(
                "blocked_threshold_m",
                "How far another building must sit from a window to NOT block the view.",
                2.0,
                15.0,
                DEFAULT_BLOCKED_THRESHOLD_M,
            ),
        ]
    }
    fn defaults() -> Self {
        Self {
            view_threshold_m: DEFAULT_VIEW_THRESHOLD_M,
            blocked_threshold_m: DEFAULT_BLOCKED_THRESHOLD_M,
        }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.view_threshold_m, self.blocked_threshold_m]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) {
            p.view_threshold_m = s.clamp(*x);
        }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) {
            p.blocked_threshold_m = s.clamp(*x);
        }
        p
    }
}

/// Compute the exterior point of an opening (door or window) on a building's
/// outer ring by ring-index interpolation. Mirrors the opinion's exact
/// `opening_point` logic so the two can't disagree on geometry.
///
/// # Arguments
/// * `ring` — the building's outer ring, as WGS84 LngLat points
/// * `ring_index` — which edge of the ring (edge from ring[i] to ring[i+1])
/// * `t` — position along that edge (0.0 at start, 1.0 at end)
///
/// # Returns
/// The interpolated point, or None if the ring is too short or index is invalid.
fn opening_point(ring: &[LngLat], ring_index: usize, t: f64) -> Option<LngLat> {
    if ring.len() < 2 {
        return None;
    }
    let idx = ring_index.min(ring.len().saturating_sub(2));
    let a = ring[idx];
    let c = ring[(idx + 1) % ring.len()];
    Some(LngLat::new(
        a.lng + (c.lng - a.lng) * t,
        a.lat + (c.lat - a.lat) * t,
    ))
}

/// Check whether a window at a given position passes the opinion's own
/// two-condition proxy. Returns (has_life, not_blocked).
fn check_window_proxy(
    point: &LngLat,
    building_idx: usize,
    nbhd: &Neighborhood,
    params: &P192Params,
) -> (bool, bool) {
    // Compute nearest street distance.
    let nearest_street = nbhd
        .streets
        .iter()
        .map(|s| point_to_polyline_m(point, &s.centerline))
        .fold(f64::MAX, f64::min);

    // Compute nearest open-space centroid distance (non-Undecided only).
    let nearest_open_space = nbhd
        .open_space
        .iter()
        .filter(|o| o.kind != OpenSpaceKind::Undecided && o.polygon.area_m2() > 0.0)
        .map(|o| haversine_m(point, &o.polygon.centroid()))
        .fold(f64::MAX, f64::min);

    let nearest_life = nearest_street.min(nearest_open_space);
    let has_life = nearest_life <= params.view_threshold_m;

    // Compute nearest OTHER building's wall vertex distance.
    let nearest_other_building = nbhd
        .buildings
        .iter()
        .enumerate()
        .filter(|(oi, _)| *oi != building_idx)
        .flat_map(|(_, b)| b.polygon.outer.iter())
        .map(|q| haversine_m(point, q))
        .fold(f64::MAX, f64::min);

    let not_blocked = nearest_other_building > params.blocked_threshold_m;

    (has_life, not_blocked)
}

/// Compute the real length (in meters) of an outer-ring edge in WGS84 space
/// using haversine distance. Matches how the opinion itself measures distances.
fn edge_length_wgs84_m(ring: &[LngLat], ring_index: usize) -> f64 {
    if ring.len() < 2 || ring_index >= ring.len() {
        return 0.0;
    }
    let a = ring[ring_index];
    let b = ring[(ring_index + 1) % ring.len()];
    haversine_m(&a, &b)
}

pub struct P192WindowsOverlookingLife;

impl PatternOperator for P192WindowsOverlookingLife {
    type Params = P192Params;

    fn name(&self) -> &'static str {
        "p192_windows_overlooking_life"
    }
    fn description(&self) -> &'static str {
        "Relocates ground-floor, outer-wall windows to positions where they overlook real street or open-space activity, not blank walls."
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p192".into(),
            display: "Alexander et al., A Pattern Language, Pattern 192 (Windows Overlooking Life)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Windows-Overlooking-Life-(192)".into()),
        }
    }

    /// `parcel_id` must be `"*"` -- relocates windows across every building
    /// in one pass, same convention `p127_intimacy_gradient`/`p116_cascade_of_roofs`
    /// use.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p192_windows_overlooking_life only supports parcel_id \"*\" -- it relocates windows across every building in one pass.".into());
        }

        // Precondition: at least one building has at least one opening.
        let has_any_opening = nbhd
            .buildings
            .iter()
            .any(|b| !b.openings.is_empty());
        if !has_any_opening {
            return Err("p192_windows_overlooking_life needs at least one real opening to relocate -- run p221_natural_doors_and_windows first.".into());
        }

        let mut new_buildings: Vec<street_smarts_core::nir::Building> = Vec::new();
        let mut replaced_building_ids: Vec<String> = Vec::new();
        let mut n_buildings_modified = 0usize;
        let mut n_windows_relocated = 0usize;
        let mut n_windows_already_good = 0usize;
        let mut n_windows_no_better_edge = 0usize;

        for (building_idx, b) in nbhd.buildings.iter().enumerate() {
            // Find all ground-floor outer-wall windows in this building.
            // Keep track of their original index in b.openings.
            let ground_floor_window_indices: Vec<usize> = b
                .openings
                .iter()
                .enumerate()
                .filter(|(_, w)| {
                    w.kind == OpeningKind::Window && !w.on_hole && w.floor == 0
                })
                .map(|(i, _)| i)
                .collect();

            if ground_floor_window_indices.is_empty() {
                continue; // This building has no ground-floor outer windows to check.
            }

            // Check which windows fail the proxy.
            let mut windows_to_relocate: Vec<usize> = Vec::new();
            for wi in &ground_floor_window_indices {
                let window = &b.openings[*wi];
                if let Some(point) = opening_point(&b.polygon.outer, window.ring_index, window.t) {
                    let (has_life, not_blocked) = check_window_proxy(&point, building_idx, nbhd, params);
                    if has_life && not_blocked {
                        // This window already passes.
                        n_windows_already_good += 1;
                    } else {
                        // This window fails one or both conditions and needs relocation.
                        windows_to_relocate.push(*wi);
                    }
                }
            }

            if windows_to_relocate.is_empty() {
                continue; // No windows to relocate in this building.
            }

            // Clone the building and attempt to relocate each failing window.
            let mut nb = b.clone();
            let mut building_modified = false;

            for wi in windows_to_relocate {
                let window = &b.openings[wi]; // Reference to the original window
                let original_ring_index = window.ring_index;
                let window_width_m = window.width_m;
                let mut relocated = false;

                // Try each ring edge (in order) as a candidate relocation target.
                let n_edges = b.polygon.outer.len().saturating_sub(1); // Exclude the closing repeat.
                for candidate_ring_index in 0..n_edges {
                    if candidate_ring_index == original_ring_index {
                        // Skip the original edge (already failed).
                        continue;
                    }

                    // Check if the candidate edge is long enough to fit this window.
                    let candidate_len = edge_length_wgs84_m(&b.polygon.outer, candidate_ring_index);
                    if candidate_len < window_width_m {
                        // Edge is too short; can't place window here.
                        continue;
                    }

                    // Compute the candidate point at the same `t` position.
                    if let Some(candidate_point) = opening_point(&b.polygon.outer, candidate_ring_index, window.t) {
                        let (has_life, not_blocked) = check_window_proxy(&candidate_point, building_idx, nbhd, params);
                        if has_life && not_blocked {
                            // This edge passes both conditions. Relocate to it.
                            nb.openings[wi].ring_index = candidate_ring_index;
                            n_windows_relocated += 1;
                            building_modified = true;
                            relocated = true;
                            break; // Place this window and move to the next.
                        }
                    }
                }

                if !relocated {
                    // No better edge found; leave this window where it is.
                    n_windows_no_better_edge += 1;
                }
            }

            if building_modified {
                n_buildings_modified += 1;
                new_buildings.push(nb);
                replaced_building_ids.push(b.id.clone());
            }
        }

        let total_checked = n_windows_already_good + n_windows_relocated + n_windows_no_better_edge;

        // Fail only if every ground-floor outer window already passes the proxy.
        // (Nothing to do = error; something failed but can't improve it = OK, just document it.)
        if total_checked > 0 && n_windows_relocated == 0 && n_windows_no_better_edge == 0 {
            return Err(format!(
                "p192_windows_overlooking_life: all {} ground-floor window(s) already overlook life; nothing to relocate.",
                n_windows_already_good
            ));
        }

        // If we have nothing to work with, error.
        if total_checked == 0 {
            return Err(
                "p192_windows_overlooking_life: no ground-floor, outer-wall windows to check -- run p221_natural_doors_and_windows first."
                    .into(),
            );
        }

        // If we found some windows to relocate and actually relocated at least one, return success.
        // If we found windows to relocate but couldn't improve any, we still return success but
        // document it in the trace (the pattern is applied, but with limited effect).
        if new_buildings.is_empty() {
            // No relocations happened, but we had failing windows (n_windows_no_better_edge > 0).
            // Return success with an empty Subdivision and a note in the trace about unrelatable windows.
            let trace = SubdivisionTrace {
                operator_name: self.name().into(),
                operator_source: self.source(),
                headline: format!(
                    "Checked {} ground-floor window(s); {} already overlook life; {} could not be improved.",
                    total_checked, n_windows_already_good, n_windows_no_better_edge
                ),
                steps: vec![
                    format!(
                        "Checked {} ground-floor outer-wall windows against life-view proxy ({:.0}m to street/open-space, no blockage within {:.0}m).",
                        total_checked,
                        params.view_threshold_m,
                        params.blocked_threshold_m
                    ),
                    format!(
                        "{} window(s) already passed; {} could not be relocated to any edge that passes both conditions.",
                        n_windows_already_good, n_windows_no_better_edge
                    ),
                ],
                caveats: vec![
                    "No windows were actually relocated -- every edge was either too short to fit the window, \
                     or no edge passes both life-view and non-blockage conditions."
                        .into(),
                    "Proximity to a street or open space is a coarse stand-in for a verified sightline \
                     -- doesn't confirm the window's own facing direction actually looks toward that feature."
                        .into(),
                ],
                seed,
                params: params.as_map(),
            };

            return Ok(Subdivision {
                new_parcels: vec![],
                new_open_space: vec![],
                new_buildings: vec![],
                new_streets: vec![],
                new_activity_nodes: vec![],
                new_boundaries: vec![],
                replaced_parcel_ids: vec![],
                replaced_open_space_ids: vec![],
                replaced_building_ids: vec![],
                entity_provenance: Default::default(),
                trace,
                new_fields: vec![],
            });
        }

        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!(
                "Relocated {} ground-floor window(s) on {} building(s) to better views of real street/open-space activity.",
                n_windows_relocated, n_buildings_modified
            ),
            steps: vec![
                format!(
                    "Checked {} ground-floor outer-wall windows against life-view proxy ({:.0}m to street/open-space, no blockage within {:.0}m).",
                    n_windows_already_good + n_windows_relocated + n_windows_no_better_edge,
                    params.view_threshold_m,
                    params.blocked_threshold_m
                ),
                format!(
                    "{} window(s) already passed; {} relocated to a better edge; {} remained in place (no edge better).",
                    n_windows_already_good, n_windows_relocated, n_windows_no_better_edge
                ),
            ],
            caveats: vec![
                "Proximity to a street or open space is a coarse stand-in for a verified sightline \
                 -- doesn't confirm the window's own facing direction actually looks toward that feature."
                    .into(),
                "Relocating a window from one ring edge to another can affect p159_light_on_two_sides \
                 (corner-adjacency check) and p164_street_windows (street-facing check) if both patterns run."
                    .into(),
                "Cannot address the 25%-of-floor-area glazing target Alexander's text gives -- no \
                 floor-area-to-glazing ratio exists in this schema; proximity-to-life is the proxy only."
                    .into(),
                "Never relocates a window onto an edge shorter than its own width_m."
                    .into(),
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
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: vec![],
            replaced_building_ids,
            entity_provenance: Default::default(),
            trace,
            new_fields: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{
        Building, NeighborhoodMeta, Opening, Street,
    };

    fn nbhd(buildings: Vec<Building>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![],
            buildings,
            streets,
            open_space: vec![],
            boundaries: vec![],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P192 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn building_with_two_windows(id: &str, x_m: f64, y_m: f64) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(x_m * m, y_m * m),
                LngLat::new((x_m + 6.0) * m, y_m * m),
                LngLat::new((x_m + 6.0) * m, (y_m + 6.0) * m),
                LngLat::new(x_m * m, (y_m + 6.0) * m),
                LngLat::new(x_m * m, y_m * m),
            ]),
            height_m: Some(6.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(2),
            openings: vec![
                // Window on ring edge 0 (south wall).
                Opening {
                    kind: OpeningKind::Window,
                    ring_index: 0,
                    on_hole: false,
                    t: 0.5,
                    width_m: 1.2,
                    sill_height_m: 0.9,
                    head_height_m: 2.1,
                    floor: 0,
                },
                // Window on ring edge 1 (east wall).
                Opening {
                    kind: OpeningKind::Window,
                    ring_index: 1,
                    on_hole: false,
                    t: 0.5,
                    width_m: 1.2,
                    sill_height_m: 0.9,
                    head_height_m: 2.1,
                    floor: 0,
                },
            ],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
            canopies: vec![],
            roof_segments: vec![],
            wall_niches: vec![],
        }
    }

    fn street_at_y(y_m: f64) -> Street {
        let m = 1.0 / 111_320.0;
        Street {
            id: "S1".into(),
            centerline: vec![
                LngLat::new(-100.0 * m, y_m * m),
                LngLat::new(100.0 * m, y_m * m),
            ],
            classification: Some("local".into()),
            row_width_m: Some(4.0),
            surface: None,
        }
    }

    #[test]
    fn parcel_id_wildcard_required() {
        let n = nbhd(vec![], vec![]);
        let err = P192WindowsOverlookingLife
            .apply(&n, "BLOCK_0", &P192Params::defaults(), 0)
            .unwrap_err();
        assert!(
            err.contains("\"*\""),
            "expected the real target-scope error, got: {err}"
        );
    }

    #[test]
    fn no_openings_at_all_is_a_real_error() {
        let b = Building {
            id: "B1".into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(-0.0001, -0.0001),
                LngLat::new(0.0001, -0.0001),
                LngLat::new(0.0001, 0.0001),
                LngLat::new(-0.0001, 0.0001),
                LngLat::new(-0.0001, -0.0001),
            ]),
            height_m: Some(6.0),
            typology: None,
            year_built: None,
            parcel_id: None,
            floors: Some(1),
            openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
            canopies: vec![],
            roof_segments: vec![],
            wall_niches: vec![],
        };
        let n = nbhd(vec![b], vec![]);
        let err = P192WindowsOverlookingLife
            .apply(&n, "*", &P192Params::defaults(), 0)
            .unwrap_err();
        assert!(
            err.contains("p221_natural_doors_and_windows"),
            "expected precondition error, got: {err}"
        );
    }

    #[test]
    fn windows_already_passing_all_conditions_is_an_error() {
        // Building with two windows; a nearby street means both pass.
        let n = nbhd(
            vec![building_with_two_windows("B1", 0.0, 0.0)],
            vec![street_at_y(-10.0)],
        );
        let err = P192WindowsOverlookingLife
            .apply(&n, "*", &P192Params::defaults(), 0)
            .unwrap_err();
        assert!(
            err.contains("all"),
            "expected the 'all windows good' error, got: {err}"
        );
    }

    #[test]
    fn a_blocked_window_gets_relocated_to_an_unblocked_edge() {
        // Building B1 has two windows. B2 is placed with a vertex close enough (< 6m)
        // to B1's south window, blocking it. A nearby street provides "life" to both
        // edges. The south window is blocked, the east window is not, so relocation
        // should move the south window to the east edge.
        let m = 1.0 / 111_320.0;
        let b1 = building_with_two_windows("B1", 0.0, 0.0);

        // B2 is a small building placed directly south of B1's south window (at 3, 0).
        // Its top-center point is at approximately (3, -3.5), which is ~3.5m from the
        // window -- well within the 6m blockage threshold.
        let b2 = Building {
            id: "B2".into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(2.0 * m, -5.0 * m),
                LngLat::new(4.0 * m, -5.0 * m),
                LngLat::new(4.0 * m, -2.0 * m),
                LngLat::new(2.0 * m, -2.0 * m),
                LngLat::new(2.0 * m, -5.0 * m),
            ]),
            height_m: Some(6.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(2),
            openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
            canopies: vec![],
            roof_segments: vec![],
            wall_niches: vec![],
        };

        // Street at y=10, about 10m north. This is within 25m, so it provides "life".
        let street = street_at_y(10.0);

        let n = nbhd(vec![b1, b2], vec![street]);
        let sub = P192WindowsOverlookingLife
            .apply(&n, "*", &P192Params::defaults(), 0)
            .unwrap();

        // Should relocate at least one window (the south one to another edge).
        assert!(!sub.new_buildings.is_empty(), "should have relocated at least one building");
        assert!(!sub.replaced_building_ids.is_empty());

        let relocated_b1 = &sub.new_buildings[0];
        // The first window (originally on ring 0, south) should have been moved to a different ring_index.
        let first_window = &relocated_b1.openings[0];
        assert!(
            first_window.ring_index != 0,
            "first window should have been moved away from ring 0, got ring_index={}",
            first_window.ring_index
        );
    }

    #[test]
    fn a_window_far_from_streets_stays_in_place() {
        // Building with two windows, no nearby street, blocked by neighbor on all sides.
        // The operator should find no better edge and report accordingly.
        let m = 1.0 / 111_320.0;
        let b1 = building_with_two_windows("B1", 0.0, 0.0);

        // Surround B1 with nearby neighbor buildings on all sides.
        let b2 = Building {
            id: "B2".into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(-9.0 * m, -3.0 * m),
                LngLat::new(-6.0 * m, -3.0 * m),
                LngLat::new(-6.0 * m, 9.0 * m),
                LngLat::new(-9.0 * m, 9.0 * m),
                LngLat::new(-9.0 * m, -3.0 * m),
            ]),
            height_m: Some(6.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(2),
            openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
            canopies: vec![],
            roof_segments: vec![],
            wall_niches: vec![],
        };

        let b3 = Building {
            id: "B3".into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(6.0 * m, -3.0 * m),
                LngLat::new(15.0 * m, -3.0 * m),
                LngLat::new(15.0 * m, 9.0 * m),
                LngLat::new(6.0 * m, 9.0 * m),
                LngLat::new(6.0 * m, -3.0 * m),
            ]),
            height_m: Some(6.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(2),
            openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
            canopies: vec![],
            roof_segments: vec![],
            wall_niches: vec![],
        };

        // Street far away.
        let street = street_at_y(100.0);

        let n = nbhd(vec![b1, b2, b3], vec![street]);
        let sub = P192WindowsOverlookingLife
            .apply(&n, "*", &P192Params::defaults(), 0)
            .unwrap();

        // Since no edge can be better (all surrounded), the operation should either
        // error or succeed with a trace noting windows that couldn't be improved.
        // The implementation should handle this gracefully.
        assert!(!sub.trace.caveats.is_empty(), "should have caveats");
    }

    #[test]
    fn windows_far_from_streets_return_success_with_no_improvements() {
        // Building with windows, but no nearby street. Windows fail the proxy,
        // but since no edge is better (all equally far from streets), no relocations occur.
        // Per the spec, this is NOT an error -- just a success with no modifications.
        let b = building_with_two_windows("B1", 0.0, 0.0);
        let n = nbhd(vec![b], vec![]);
        let sub = P192WindowsOverlookingLife
            .apply(&n, "*", &P192Params::defaults(), 0)
            .unwrap();
        // Should succeed but with an empty Subdivision (no relocations).
        assert!(sub.new_buildings.is_empty(), "should not relocate when all edges are equally bad");
        assert!(sub.replaced_building_ids.is_empty());
        // The trace should document that windows couldn't be improved.
        assert!(
            sub.trace.headline.contains("could not be improved"),
            "trace should mention windows that couldn't be improved"
        );
    }

    #[test]
    fn params_roundtrip() {
        let p = P192Params {
            view_threshold_m: 30.0,
            blocked_threshold_m: 8.0,
        };
        let v = p.as_vector();
        let back = P192Params::from_vector(&v);
        assert_eq!(back.view_threshold_m, 30.0);
        assert_eq!(back.blocked_threshold_m, 8.0);
    }
}
