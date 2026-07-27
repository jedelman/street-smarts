//! P110 Main Entrance — a building's entrance should be immediately visible
//! from the main paths of approach, not buried on a far side wall.
//!
//! From Alexander, *A Pattern Language*, Pattern 110, via
//! patternlanguage.cc/Patterns/Main-Entrance-(110):
//! > **Problem:** Placing the main entrance (or main entrances) is perhaps
//! > the single most important step you take during the evolution of a
//! > building plan.
//! > **Solution:** Place the main entrance of the building at a point
//! > where it can be seen immediately from the main avenues of approach
//! > and give it a bold, visible shape which stands out in front of the
//! > building.
//!
//! # The same real proxy the opinion already checks
//!
//! `crates/street-smarts-opinions/src/pattern/p110_main_entrance.rs` (the
//! detector for this same pattern) has, since it shipped, checked two aspects:
//! - `singularity`: scored `1.0 / (count of ground-floor outer doors)` per
//!   building, favoring exactly one entrance.
//! - `street_proximity`: scored based on straight-line distance from the
//!   building's first qualifying ground-floor outer door to the nearest real
//!   `Street` centerline VERTEX, proxying "seen immediately from the main
//!   avenues of approach" at `visible_threshold_m` (default 40m), falling to
//!   0 by double that distance.
//!
//! This operator closes the gap on `street_proximity` ONLY: for each building
//! that has at least one ground-floor, outer-wall door, look at the FIRST such
//! door (matching the opinion's own convention). Compute its distance to the
//! nearest real `Street` centerline vertex. If that distance exceeds
//! `visible_threshold_m`, RELOCATE that one door to a different edge of the
//! SAME building's outer ring — specifically, the outer-ring edge whose
//! midpoint is closest to the nearest street point — by changing its
//! `ring_index` to that edge's index and setting `t = 0.5`. Keep all other
//! properties unchanged. Use the EXACT same ring-interpolation and
//! haversine-distance logic the opinion does, so the two can't disagree about
//! geometry.
//!
//! # What this deliberately does NOT do
//!
//! - **Does NOT address `singularity` (multi-entrance buildings).** Alexander's
//!   "the main entrance" suggests a single prominent door. However, this schema
//!   has no "is_primary" marker on `Opening`, and deleting other doors to
//!   achieve singularity would regress door-count-dependent opinions elsewhere
//!   (P100 Pedestrian Street, P112, P115, P165 all care about door presence
//!   and coverage). Consolidating doors is out of scope. Only the FIRST
//!   qualifying ground-floor outer door per building is relocated; any
//!   additional doors beyond doors[0] are left untouched.
//! - **No real sightline verification.** Same honest limitation the opinion's
//!   own caveat already states: proximity to a street is a coarse stand-in for
//!   "seen immediately" -- doesn't verify the entrance face actually points
//!   toward the street or that no visual obstacles block the sightline.
//! - **No "bold, visible shape" check.** Alexander's text calls for an
//!   entrance that "stands out in front of the building" visually. No
//!   architectural-massing or facade-articulation data exists in this schema
//!   beyond door width. Checking door width alone isn't a real proxy for
//!   visual prominence, so this was not attempted (matches the opinion's own
//!   honest caveat).
//!
//! # Required cross-pattern caveat
//!
//! **Run this generator BEFORE `p102_family_of_entrances` generators**, if any
//! exist. Relocating a door changes its real-world position, which affects
//! P102's `clustering` sub-score (proximity between different buildings'
//! entrances) for OTHER buildings too. Measure entrance clustering against
//! final positions, not stale ones.

use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::{haversine_m, LngLat};
use street_smarts_core::nir::{Neighborhood, OpeningKind};
use street_smarts_core::opinion::SourceCitation;

/// Matches the real opinion's own `VISIBLE_THRESHOLD_M` exactly -- see
/// this module's own doc for why that agreement matters.
const DEFAULT_VISIBLE_THRESHOLD_M: f64 = 40.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P110Params {
    /// How close a real street centerline vertex must sit to a door's
    /// exterior point to count as "immediately visible." Must match the
    /// opinion's own `VISIBLE_THRESHOLD_M`.
    pub visible_threshold_m: f64,
}

impl Parameters for P110Params {
    fn schema() -> Vec<ParamSpec> {
        vec![ParamSpec::float(
            "visible_threshold_m",
            "How close a street centerline vertex must sit to a door to count as immediately visible.",
            20.0,
            60.0,
            DEFAULT_VISIBLE_THRESHOLD_M,
        )
        .with_unit("m")]
    }

    fn defaults() -> Self {
        Self {
            visible_threshold_m: DEFAULT_VISIBLE_THRESHOLD_M,
        }
    }

    fn as_vector(&self) -> Vec<f64> {
        vec![self.visible_threshold_m]
    }

    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) {
            p.visible_threshold_m = s.clamp(*x);
        }
        p
    }
}

/// Compute the exterior point of a door on a building's outer ring by
/// ring-index interpolation. Mirrors the opinion's exact logic so the two
/// can't disagree on geometry.
///
/// # Arguments
/// * `ring` — the building's outer ring, as WGS84 LngLat points
/// * `ring_index` — which edge of the ring (edge from ring[i] to ring[i+1])
/// * `t` — position along that edge (0.0 at start, 1.0 at end)
///
/// # Returns
/// The interpolated point, or None if the ring is too short or index is invalid.
fn door_point(ring: &[LngLat], ring_index: usize, t: f64) -> Option<LngLat> {
    if ring.len() < 2 {
        return None;
    }
    if ring_index + 1 >= ring.len() {
        return None;
    }
    let a = ring[ring_index];
    let c = ring[ring_index + 1];
    Some(LngLat::new(
        a.lng + (c.lng - a.lng) * t,
        a.lat + (c.lat - a.lat) * t,
    ))
}

/// Compute the midpoint of an outer-ring edge in WGS84 space.
fn edge_midpoint(ring: &[LngLat], ring_index: usize) -> Option<LngLat> {
    door_point(ring, ring_index, 0.5)
}

pub struct P110MainEntrance;

impl PatternOperator for P110MainEntrance {
    type Params = P110Params;

    fn name(&self) -> &'static str {
        "p110_main_entrance"
    }

    fn description(&self) -> &'static str {
        "Relocates a building's first ground-floor door closer to the nearest real street for better street visibility."
    }

    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p110".into(),
            display: "Alexander et al., A Pattern Language, Pattern 110 (Main Entrance)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Main-Entrance-(110)".into()),
        }
    }

    /// `parcel_id` must be `"*"` -- relocates doors across every building
    /// in one pass.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err(
                "p110_main_entrance only supports parcel_id \"*\" -- it relocates doors across every building in one pass."
                    .into(),
            );
        }

        // Precondition: at least one building.
        if nbhd.buildings.is_empty() {
            return Err("No buildings in this neighborhood to check for entrance visibility.".into());
        }

        // Precondition: at least one street.
        if nbhd.streets.is_empty() {
            return Err(
                "p110_main_entrance needs real streets to check proximity -- nothing to measure \
                 visibility against."
                    .into(),
            );
        }

        // Collect all street vertices to measure against.
        let street_vertices: Vec<LngLat> =
            nbhd.streets.iter().flat_map(|s| s.centerline.iter().copied()).collect();

        let mut new_buildings: Vec<street_smarts_core::nir::Building> = Vec::new();
        let mut replaced_building_ids: Vec<String> = Vec::new();
        let mut n_buildings_checked = 0usize;
        let mut n_buildings_relocated = 0usize;
        let mut n_doors_already_close = 0usize;

        for b in &nbhd.buildings {
            // Find all ground-floor, outer-wall doors in this building.
            let qualifying_doors: Vec<usize> = b
                .openings
                .iter()
                .enumerate()
                .filter(|(_, o)| o.kind == OpeningKind::Door && !o.on_hole && o.floor == 0)
                .map(|(i, _)| i)
                .collect();

            if qualifying_doors.is_empty() {
                // No doors to check in this building.
                continue;
            }

            n_buildings_checked += 1;

            // Work with only the FIRST qualifying door, matching the opinion's own convention.
            let door_idx = qualifying_doors[0];
            let door = &b.openings[door_idx];
            let ring = &b.polygon.outer;

            // Compute the current door position.
            if let Some(current_pos) = door_point(ring, door.ring_index, door.t) {
                // Find the nearest street vertex.
                let nearest_m = street_vertices
                    .iter()
                    .map(|v| haversine_m(&current_pos, v))
                    .fold(f64::MAX, f64::min);

                // Check if already within threshold.
                if nearest_m <= params.visible_threshold_m {
                    n_doors_already_close += 1;
                    continue;
                }

                // Door is too far. Find the street vertex that's nearest.
                let nearest_street_vertex = street_vertices
                    .iter()
                    .copied()
                    .min_by(|a, b| {
                        haversine_m(&current_pos, a)
                            .partial_cmp(&haversine_m(&current_pos, b))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .unwrap();

                // Find the outer-ring edge whose midpoint is closest to the nearest street vertex.
                let n_edges = ring.len().saturating_sub(1);
                let mut best_ring_index = door.ring_index;
                let mut best_distance = f64::MAX;

                for candidate_idx in 0..n_edges {
                    if let Some(midpoint) = edge_midpoint(ring, candidate_idx) {
                        let dist = haversine_m(&midpoint, &nearest_street_vertex);
                        if dist < best_distance {
                            best_distance = dist;
                            best_ring_index = candidate_idx;
                        }
                    }
                }

                // Relocate the door to the best edge.
                let mut nb = b.clone();
                nb.openings[door_idx].ring_index = best_ring_index;
                nb.openings[door_idx].t = 0.5;

                n_buildings_relocated += 1;
                new_buildings.push(nb);
                replaced_building_ids.push(b.id.clone());
            }
        }

        // Precondition: at least one building had a qualifying door.
        if n_buildings_checked == 0 {
            return Err(
                "No buildings have a ground-floor, outer-wall door yet -- run P221 Natural Doors and Windows first."
                    .into(),
            );
        }

        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: if n_buildings_relocated > 0 {
                format!(
                    "Relocated {} building(s)' main entrance closer to nearby streets for better visibility.",
                    n_buildings_relocated
                )
            } else {
                format!(
                    "Checked {} building(s) for entrance visibility; {} already within {:.0}m of a street.",
                    n_buildings_checked, n_doors_already_close, params.visible_threshold_m
                )
            },
            steps: vec![format!(
                "{} building(s) checked; {} doors already within {:.0}m of a real street vertex; {} door(s) relocated to the edge nearest a street vertex.",
                n_buildings_checked, n_doors_already_close, params.visible_threshold_m, n_buildings_relocated
            )],
            caveats: vec![
                "Only relocates the FIRST qualifying (ground-floor, outer-wall) door per building; \
                 does not address singularity (multi-entrance buildings) as that would regress \
                 door-count-dependent opinions elsewhere (P100, P112, P115, P165)."
                    .into(),
                "Uses straight-line distance to the nearest street CENTERLINE VERTEX as its \
                 proximity proxy, matching the opinion's own imprecision, not a real sightline/\
                 visibility check."
                    .into(),
                "Should run BEFORE any p102_family_of_entrances generators, so entrance-clustering \
                 gets measured against final door positions, not stale ones."
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
    use street_smarts_core::nir::{Building, NeighborhoodMeta, Opening, Street};

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
                label: "P110 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn rect_building(id: &str, openings: Vec<Opening>) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0),
                LngLat::new(10.0 * m, 0.0),
                LngLat::new(10.0 * m, 10.0 * m),
                LngLat::new(0.0, 10.0 * m),
                LngLat::new(0.0, 0.0),
            ]),
            height_m: Some(6.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(2),
            openings,
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
            canopies: vec![],
            roof_segments: vec![],
            wall_niches: vec![],
        }
    }

    fn door() -> Opening {
        Opening {
            kind: OpeningKind::Door,
            ring_index: 0,
            on_hole: false,
            t: 0.5,
            width_m: 0.9,
            sill_height_m: 0.0,
            head_height_m: 2.1,
            floor: 0,
        }
    }

    #[test]
    fn no_buildings_is_err() {
        let params = P110Params::defaults();
        let result = P110MainEntrance.apply(&nbhd(vec![], vec![]), "*", &params, 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No buildings"));
    }

    #[test]
    fn no_qualifying_doors_is_err() {
        let m = 1.0 / 111_320.0;
        let street = Street {
            id: "S1".into(),
            centerline: vec![LngLat::new(5.0 * m, 5.0 * m), LngLat::new(5.0 * m, -5.0 * m)],
            classification: Some("local".into()),
            row_width_m: Some(8.0),
            surface: None,
        };
        let params = P110Params::defaults();
        let result = P110MainEntrance.apply(&nbhd(vec![rect_building("B1", vec![])], vec![street]), "*", &params, 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("P221"));
    }

    #[test]
    fn no_streets_is_err() {
        let params = P110Params::defaults();
        let result = P110MainEntrance.apply(&nbhd(vec![rect_building("B1", vec![door()])], vec![]), "*", &params, 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("streets"));
    }

    #[test]
    fn door_already_close_to_street_not_relocated() {
        let m = 1.0 / 111_320.0;
        // Building at (0, 0) to (10m, 10m).
        let building = rect_building("B1", vec![door()]);
        // Street passes very close to the building (within visible threshold).
        let street = Street {
            id: "S1".into(),
            centerline: vec![LngLat::new(5.0 * m, -2.0 * m), LngLat::new(5.0 * m, -1.0 * m)],
            classification: Some("local".into()),
            row_width_m: Some(8.0),
            surface: None,
        };
        let params = P110Params::defaults();
        let result =
            P110MainEntrance.apply(&nbhd(vec![building], vec![street]), "*", &params, 0);
        assert!(result.is_ok());
        let sub = result.unwrap();
        assert_eq!(sub.new_buildings.len(), 0, "door already close should not be relocated");
        assert_eq!(sub.replaced_building_ids.len(), 0);
    }

    #[test]
    fn door_far_from_street_gets_relocated() {
        let m = 1.0 / 111_320.0;
        // Building at (0, 0) to (10m, 10m) in grid units.
        let building = rect_building("B1", vec![door()]);
        // Street is far away (well beyond visible threshold).
        let street = Street {
            id: "S1".into(),
            centerline: vec![LngLat::new(100.0 * m, 100.0 * m), LngLat::new(100.0 * m, 50.0 * m)],
            classification: Some("local".into()),
            row_width_m: Some(8.0),
            surface: None,
        };
        let params = P110Params::defaults();
        let result =
            P110MainEntrance.apply(&nbhd(vec![building.clone()], vec![street]), "*", &params, 0);
        assert!(result.is_ok());
        let sub = result.unwrap();
        assert_eq!(sub.new_buildings.len(), 1, "door far from street should be relocated");
        assert_eq!(sub.replaced_building_ids.len(), 1);
        assert_eq!(sub.replaced_building_ids[0], "B1");

        // Verify the door was actually moved to a different edge.
        let relocated_building = &sub.new_buildings[0];
        assert_ne!(
            relocated_building.openings[0].ring_index, building.openings[0].ring_index,
            "door should have been moved to a different ring edge"
        );
        assert_eq!(relocated_building.openings[0].t, 0.5, "door should be at edge midpoint");
    }

    #[test]
    fn relocated_door_is_closer_to_street() {
        let m = 1.0 / 111_320.0;
        // Building at (0, 0) to (10m, 10m).
        let building = rect_building("B1", vec![door()]);
        // Street is far away.
        let street = Street {
            id: "S1".into(),
            centerline: vec![LngLat::new(100.0 * m, 100.0 * m), LngLat::new(100.0 * m, 50.0 * m)],
            classification: Some("local".into()),
            row_width_m: Some(8.0),
            surface: None,
        };
        let params = P110Params::defaults();
        let result =
            P110MainEntrance.apply(&nbhd(vec![building.clone()], vec![street.clone()]), "*", &params, 0);
        assert!(result.is_ok());
        let sub = result.unwrap();

        // Compute the distance from the original door position to the street.
        let original_door_pos =
            door_point(&building.polygon.outer, building.openings[0].ring_index, building.openings[0].t)
                .unwrap();
        let original_distance = street
            .centerline
            .iter()
            .map(|v| haversine_m(&original_door_pos, v))
            .fold(f64::MAX, f64::min);

        // Compute the distance from the relocated door position to the street.
        let relocated_building = &sub.new_buildings[0];
        let relocated_door_pos = door_point(
            &relocated_building.polygon.outer,
            relocated_building.openings[0].ring_index,
            relocated_building.openings[0].t,
        )
        .unwrap();
        let relocated_distance = street
            .centerline
            .iter()
            .map(|v| haversine_m(&relocated_door_pos, v))
            .fold(f64::MAX, f64::min);

        assert!(
            relocated_distance < original_distance,
            "relocated door should be closer to street: {:.2}m vs {:.2}m",
            relocated_distance,
            original_distance
        );
    }

    #[test]
    fn mixed_near_and_far_buildings_only_far_ones_relocated() {
        let m = 1.0 / 111_320.0;
        // Building B1 at (0, 0) to (10m, 10m), door on edge 0 (south).
        let b1 = rect_building("B1", vec![door()]);
        // Building B2 at (50m, 50m) to (60m, 60m), door on edge 0 (south).
        let b2 = {
            let mut b = rect_building("B2", vec![door()]);
            b.polygon = Polygon::from_ring(vec![
                LngLat::new(50.0 * m, 50.0 * m),
                LngLat::new(60.0 * m, 50.0 * m),
                LngLat::new(60.0 * m, 60.0 * m),
                LngLat::new(50.0 * m, 60.0 * m),
                LngLat::new(50.0 * m, 50.0 * m),
            ]);
            b
        };

        // Street is near B1 but far from B2.
        let street = Street {
            id: "S1".into(),
            centerline: vec![LngLat::new(5.0 * m, -5.0 * m), LngLat::new(5.0 * m, -1.0 * m)],
            classification: Some("local".into()),
            row_width_m: Some(8.0),
            surface: None,
        };

        let params = P110Params::defaults();
        let result =
            P110MainEntrance.apply(&nbhd(vec![b1, b2], vec![street]), "*", &params, 0);
        assert!(result.is_ok());
        let sub = result.unwrap();

        // Only B2 should be relocated.
        assert_eq!(sub.new_buildings.len(), 1);
        assert_eq!(sub.replaced_building_ids.len(), 1);
        assert_eq!(sub.replaced_building_ids[0], "B2");
    }

    #[test]
    fn multiple_doors_only_first_relocated() {
        let m = 1.0 / 111_320.0;
        // Building with two doors: one on edge 0, one on edge 1.
        let building = rect_building(
            "B1",
            vec![
                door(),
                Opening {
                    kind: OpeningKind::Door,
                    ring_index: 1,
                    on_hole: false,
                    t: 0.5,
                    width_m: 0.9,
                    sill_height_m: 0.0,
                    head_height_m: 2.1,
                    floor: 0,
                },
            ],
        );
        // Street is far away.
        let street = Street {
            id: "S1".into(),
            centerline: vec![LngLat::new(100.0 * m, 100.0 * m), LngLat::new(100.0 * m, 50.0 * m)],
            classification: Some("local".into()),
            row_width_m: Some(8.0),
            surface: None,
        };

        let params = P110Params::defaults();
        let result =
            P110MainEntrance.apply(&nbhd(vec![building.clone()], vec![street]), "*", &params, 0);
        assert!(result.is_ok());
        let sub = result.unwrap();

        assert_eq!(sub.new_buildings.len(), 1);
        let relocated_building = &sub.new_buildings[0];

        // First door should be relocated.
        assert_ne!(relocated_building.openings[0].ring_index, building.openings[0].ring_index);
        // Second door should remain unchanged.
        assert_eq!(relocated_building.openings[1].ring_index, building.openings[1].ring_index);
        assert_eq!(relocated_building.openings[1].t, building.openings[1].t);
    }
}
