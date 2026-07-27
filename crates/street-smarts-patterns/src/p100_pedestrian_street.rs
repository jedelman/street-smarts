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
//! # The same real proxy the opinion already checks
//!
//! `crates/street-smarts-opinions/src/pattern/p100_pedestrian_street.rs`
//! (the detector for this same pattern) has, since it shipped, scored a real
//! `Pedestrian`-classified street as satisfactory when ground-floor outer-wall
//! entrances (`Opening { kind: Door, on_hole: false, floor: 0 }`) within
//! `adjacency_threshold_m` of that street's centerline reach
//! `min_entrances_per_100m` density (default 3.0 per 100m). This operator
//! closes that gap using the EXACT same definition (same 20m default threshold,
//! same "first ground-floor outer door per building" measure): for each real
//! `Pedestrian` street scoring below 1.0, find buildings with NO existing
//! ground-floor outer door, check if they have an outer ring edge within
//! threshold, and add a new door to the nearest candidate on the nearest edge
//! until the street's entrance density target is met. The generator and the
//! opinion can't silently disagree about what counts as a satisfying entrance,
//! because they're reading the same real geometry the same way.
//!
//! # What this deliberately does NOT do
//! - **No "open stairs directly from upper stories to the street".** Same
//!   honest limitation the opinion's own caveat already states: no
//!   vertical-circulation-to-exterior data exists in this schema
//!   (`InteriorCell.floor` is always 0). This adds only ground-floor doors.
//! - **Only one new door per building across the whole run.** Even when a
//!   building sits adjacent to two pedestrian streets, each building receives
//!   at most one new door. This is a real, specific choice to avoid
//!   over-densifying a single façade when multiple streets are being scored.
//! - **No entrance-position sophistication.** Places the door at the edge
//!   midpoint (`t: 0.5`), not at a real, considered entrance position (e.g.,
//!   a corner, an existing fenestration rhythm, or a street crossing).
//! - **P110 (Main Entrance) tension.** P110 scores `singularity = 1.0 / (count
//!   of ground-floor outer doors)` per building. This operator only adds doors
//!   to buildings that had none, so it does not reduce any building's
//!   singularity. However, in general, P100 (many entrances) and P110 (one
//!   prominent entrance) pull in opposite directions — a future version that
//!   added SECOND doors to existing buildings would lower P110's score.

use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use street_smarts_core::geometry::{haversine_m, point_to_polyline_m, LngLat};
use street_smarts_core::nir::{Building, Neighborhood, Opening, OpeningKind};
use street_smarts_core::opinion::SourceCitation;

/// Matches the real opinion's own `ADJACENCY_THRESHOLD_M` exactly.
const DEFAULT_ADJACENCY_THRESHOLD_M: f64 = 20.0;
/// Matches the real opinion's own `MIN_ENTRANCES_PER_100M` exactly.
const DEFAULT_MIN_ENTRANCES_PER_100M: f64 = 3.0;
/// Default ground-floor door width in meters.
const DEFAULT_DOOR_WIDTH_M: f64 = 0.9;
/// Default ground-floor door head height in meters.
const DEFAULT_DOOR_HEAD_HEIGHT_M: f64 = 2.1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P100Params {
    /// How close a building's outer-ring edge midpoint must sit to a
    /// pedestrian street's centerline to be considered adjacent.
    pub adjacency_threshold_m: f64,
    /// Minimum entrance density (per 100 meters of street length) required
    /// to satisfy this pattern. Matches the opinion's own target exactly.
    pub min_entrances_per_100m: f64,
    /// Width of each new door opening, in meters.
    pub door_width_m: f64,
    /// Head height of each new door opening, in meters.
    pub door_head_height_m: f64,
}

impl Parameters for P100Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "adjacency_threshold_m",
                "How close a building's outer-ring edge midpoint must sit to a pedestrian street's centerline to be considered adjacent.",
                5.0,
                50.0,
                DEFAULT_ADJACENCY_THRESHOLD_M,
            ).with_unit("m"),
            ParamSpec::float(
                "min_entrances_per_100m",
                "Minimum entrance density required to satisfy the pedestrian street pattern.",
                1.0,
                10.0,
                DEFAULT_MIN_ENTRANCES_PER_100M,
            ),
            ParamSpec::float(
                "door_width_m",
                "Width of each new door opening.",
                0.6,
                2.5,
                DEFAULT_DOOR_WIDTH_M,
            ).with_unit("m"),
            ParamSpec::float(
                "door_head_height_m",
                "Head height of each new door opening.",
                1.8,
                3.0,
                DEFAULT_DOOR_HEAD_HEIGHT_M,
            ).with_unit("m"),
        ]
    }
    fn defaults() -> Self {
        Self {
            adjacency_threshold_m: DEFAULT_ADJACENCY_THRESHOLD_M,
            min_entrances_per_100m: DEFAULT_MIN_ENTRANCES_PER_100M,
            door_width_m: DEFAULT_DOOR_WIDTH_M,
            door_head_height_m: DEFAULT_DOOR_HEAD_HEIGHT_M,
        }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![
            self.adjacency_threshold_m,
            self.min_entrances_per_100m,
            self.door_width_m,
            self.door_head_height_m,
        ]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) {
            p.adjacency_threshold_m = s.clamp(*x);
        }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) {
            p.min_entrances_per_100m = s.clamp(*x);
        }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) {
            p.door_width_m = s.clamp(*x);
        }
        if let (Some(s), Some(x)) = (schema.get(3), v.get(3)) {
            p.door_head_height_m = s.clamp(*x);
        }
        p
    }
}

pub struct P100PedestrianStreet;

impl PatternOperator for P100PedestrianStreet {
    type Params = P100Params;

    fn name(&self) -> &'static str {
        "p100_pedestrian_street"
    }
    fn description(&self) -> &'static str {
        "Adds ground-floor door entrances to buildings adjacent to pedestrian streets, targeting a walkable-frontage entrance density per P100."
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p100".into(),
            display: "Alexander et al., A Pattern Language, Pattern 100 (Pedestrian Street)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Pedestrian-Street-(100)".into()),
        }
    }

    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err(
                "p100_pedestrian_street only supports parcel_id \"*\" -- it adds doors to every pedestrian street in one pass.".into(),
            );
        }

        // Filter to pedestrian streets only (exact match to opinion).
        let ped_streets: Vec<_> = nbhd
            .streets
            .iter()
            .filter(|s| s.classification.as_deref() == Some("pedestrian"))
            .collect();
        if ped_streets.is_empty() {
            return Err(
                "p100_pedestrian_street needs at least one Pedestrian-classified street -- run path_network first.".into(),
            );
        }

        // Compute street lengths and find streets that are below the target.
        let mut streets_to_improve: Vec<_> = Vec::new();
        for s in &ped_streets {
            let street_len: f64 = s.centerline.windows(2).map(|w| haversine_m(&w[0], &w[1])).sum();
            if street_len <= 0.0 {
                continue;
            }
            streets_to_improve.push((*s, street_len));
        }

        if streets_to_improve.is_empty() {
            return Err(
                "p100_pedestrian_street: no pedestrian street had positive real length to measure.".into(),
            );
        }

        // Build a map of which buildings have no ground-floor outer door.
        let doorless_buildings: Vec<_> = nbhd
            .buildings
            .iter()
            .filter(|b| {
                !b.openings.iter().any(|o| {
                    o.kind == OpeningKind::Door && !o.on_hole && o.floor == 0
                })
            })
            .collect();

        if doorless_buildings.is_empty() {
            return Err(
                "p100_pedestrian_street: every building already has a ground-floor outer-wall entrance -- nothing to add.".into(),
            );
        }

        // Track which buildings we've already given a door to (to enforce
        // one-door-per-building rule across the whole run).
        let mut buildings_given_doors: HashSet<String> = HashSet::new();
        let mut new_buildings: Vec<Building> = Vec::new();
        let mut replaced_building_ids: Vec<String> = Vec::new();

        let mut total_doors_added = 0usize;
        let mut streets_improved: Vec<String> = Vec::new();

        // For each pedestrian street that is below target...
        for (street, street_len) in streets_to_improve {
            // Compute current entrance count (opinion logic: first door per building).
            let mut current_entrance_count = 0usize;
            for b in &nbhd.buildings {
                if let Some(_door) = b.openings.iter().find(|o| {
                    o.kind == OpeningKind::Door && !o.on_hole && o.floor == 0
                }) {
                    let door_pt = self.opening_to_point(b, _door)?;
                    if point_to_polyline_m(&door_pt, &street.centerline)
                        <= params.adjacency_threshold_m
                    {
                        current_entrance_count += 1;
                    }
                }
            }

            let min_needed = (params.min_entrances_per_100m * street_len / 100.0).ceil() as usize;
            if current_entrance_count >= min_needed {
                // Street already meets target.
                continue;
            }

            let mut doors_needed = min_needed - current_entrance_count;

            // Find candidate buildings (doorless + adjacent to this street).
            let mut candidates: Vec<(f64, &Building, usize)> = Vec::new();
            for b in &doorless_buildings {
                if buildings_given_doors.contains(&b.id) {
                    // Already gave this building a door in a previous iteration.
                    continue;
                }
                // Find the nearest edge midpoint to this street.
                let mut nearest_dist = f64::MAX;
                let mut nearest_edge_idx: Option<usize> = None;

                for edge_idx in 0..b.polygon.outer.len() {
                    let a = b.polygon.outer[edge_idx];
                    let c = b.polygon.outer[(edge_idx + 1) % b.polygon.outer.len()];
                    let midpoint = LngLat::new(
                        (a.lng + c.lng) / 2.0,
                        (a.lat + c.lat) / 2.0,
                    );
                    let dist = point_to_polyline_m(&midpoint, &street.centerline);
                    if dist < nearest_dist {
                        nearest_dist = dist;
                        nearest_edge_idx = Some(edge_idx);
                    }
                }

                if let Some(edge_idx) = nearest_edge_idx {
                    if nearest_dist <= params.adjacency_threshold_m {
                        candidates.push((nearest_dist, b, edge_idx));
                    }
                }
            }

            if candidates.is_empty() {
                // No doorless buildings adjacent to this street.
                continue;
            }

            // Sort by distance (nearest first, deterministic).
            candidates.sort_by(|a, b| {
                a.0.partial_cmp(&b.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.1.id.cmp(&b.1.id))
            });

            // Add doors to candidates until target is met or we run out.
            for (_dist, b, edge_idx) in candidates.iter().take(doors_needed) {
                if buildings_given_doors.contains(&b.id) {
                    continue;
                }

                // Clone the building and add a door on the nearest edge.
                let mut nb = (*b).clone();

                // Compute the edge's real length in degrees (local-projected).
                let a = nb.polygon.outer[*edge_idx];
                let c = nb.polygon.outer[(*edge_idx + 1) % nb.polygon.outer.len()];
                let edge_len_degrees = (a.lng - c.lng).hypot(a.lat - c.lat);
                let edge_len_m = edge_len_degrees * 111_320.0; // degrees to meters.

                // Clamp door width to 90% of edge length.
                let actual_door_width = params.door_width_m.min(edge_len_m * 0.9);

                nb.openings.push(Opening {
                    kind: OpeningKind::Door,
                    ring_index: *edge_idx,
                    on_hole: false,
                    t: 0.5,
                    width_m: actual_door_width,
                    sill_height_m: 0.0,
                    head_height_m: params.door_head_height_m,
                    floor: 0,
                });

                new_buildings.push(nb);
                replaced_building_ids.push(b.id.clone());
                buildings_given_doors.insert(b.id.clone());
                total_doors_added += 1;

                // Decrement doors_needed and check if we've met the target.
                doors_needed -= 1;
                if doors_needed == 0 {
                    streets_improved.push(street.id.clone());
                    break;
                }
            }
        }

        if new_buildings.is_empty() {
            return Err(
                "p100_pedestrian_street: no pedestrian street had adjacent doorless buildings within the threshold -- nothing to add.".into(),
            );
        }

        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!(
                "Added {total_doors_added} ground-floor door(s) to adjacent buildings, improving entrance density on {} pedestrian street(s).",
                streets_improved.len()
            ),
            steps: vec![format!(
                "{total_doors_added} door(s) placed at 0.5 t on the nearest outer-ring edge within {:.1}m of each improved street; {} pedestrian street(s) now meet the {:.1} per-100m entrance target.",
                params.adjacency_threshold_m,
                streets_improved.len(),
                params.min_entrances_per_100m
            )],
            caveats: vec![
                "Cannot generate 'open stairs directly from the upper stories to the street' -- Alexander's own \
                 second half of this pattern. No vertical-circulation-to-exterior data exists in this schema."
                    .into(),
                "Places the door at the edge midpoint (t: 0.5), not at a real, considered entrance position."
                    .into(),
                "Each building receives at most one new door across the whole run, even if adjacent to multiple \
                 pedestrian streets. This avoids over-densifying a single façade."
                    .into(),
                "P110 (Main Entrance, singularity per building) and P100 (many entrances) pull in opposite directions \
                 in general. This operator only adds doors to buildings that had none, so it does not reduce any \
                 building's singularity; however, a future version that added SECOND doors would lower P110's score."
                    .into(),
                "min_entrances_per_100m (3.0) is a defensible walkable-frontage target, not a figure Alexander's \
                 own text provides."
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

impl P100PedestrianStreet {
    /// Convert an Opening to its real position on the building's outer ring,
    /// using ring interpolation (same as the opinion does).
    fn opening_to_point(&self, b: &Building, o: &Opening) -> Result<LngLat, String> {
        let ring = &b.polygon.outer;
        let idx = o.ring_index.min(ring.len().saturating_sub(2));
        if idx >= ring.len() {
            return Err("opening ring_index out of bounds".into());
        }
        let a = ring[idx];
        let c = ring[(idx + 1) % ring.len()];
        Ok(LngLat::new(
            a.lng + (c.lng - a.lng) * o.t,
            a.lat + (c.lat - a.lat) * o.t,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{NeighborhoodMeta, Street};

    fn m() -> f64 {
        1.0 / 111_320.0
    }

    fn nbhd(buildings: Vec<Building>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
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
                label: "P100 generator unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn ped_street(id: &str, len_m: f64) -> Street {
        Street {
            id: id.into(),
            centerline: vec![
                LngLat::new(0.0, 0.0),
                LngLat::new(len_m * m(), 0.0),
            ],
            classification: Some("pedestrian".into()),
            row_width_m: Some(4.0),
            surface: None,
        }
    }

    fn building_without_door(id: &str, x_m: f64, y_m: f64, side_m: f64) -> Building {
        let mm = m();
        let x = x_m * mm;
        let y = y_m * mm;
        let s = (side_m / 2.0) * mm;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(x - s, y - s),
                LngLat::new(x + s, y - s),
                LngLat::new(x + s, y + s),
                LngLat::new(x - s, y + s),
                LngLat::new(x - s, y - s),
            ]),
            height_m: Some(9.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(3),
            openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
            canopies: vec![],
            roof_segments: vec![],
            wall_niches: vec![],
        }
    }

    fn building_with_door(id: &str, x_m: f64, y_m: f64, side_m: f64) -> Building {
        let mut b = building_without_door(id, x_m, y_m, side_m);
        b.openings.push(Opening {
            kind: OpeningKind::Door,
            ring_index: 0,
            on_hole: false,
            t: 0.5,
            width_m: 0.9,
            sill_height_m: 0.0,
            head_height_m: 2.1,
            floor: 0,
        });
        b
    }

    #[test]
    fn parcel_id_not_star_is_an_error() {
        let n = nbhd(vec![], vec![ped_street("PED1", 100.0)]);
        let err = P100PedestrianStreet.apply(&n, "BLOCK_0", &P100Params::defaults(), 0).unwrap_err();
        assert!(
            err.contains("\"*\""),
            "expected target-scope error, got: {err}"
        );
    }

    #[test]
    fn no_pedestrian_streets_is_an_error() {
        let local_street = Street {
            id: "LOCAL1".into(),
            centerline: vec![LngLat::new(0.0, 0.0), LngLat::new(100.0 * m(), 0.0)],
            classification: Some("local".into()),
            row_width_m: Some(5.5),
            surface: None,
        };
        let n = nbhd(vec![], vec![local_street]);
        let err = P100PedestrianStreet.apply(&n, "*", &P100Params::defaults(), 0).unwrap_err();
        assert!(
            err.contains("Pedestrian-classified street"),
            "expected 'need pedestrian street' error, got: {err}"
        );
    }

    #[test]
    fn no_positive_length_pedestrian_street_is_an_error() {
        let zero_len_street = Street {
            id: "PED_ZERO".into(),
            centerline: vec![LngLat::new(0.0, 0.0), LngLat::new(0.0, 0.0)],
            classification: Some("pedestrian".into()),
            row_width_m: Some(4.0),
            surface: None,
        };
        let n = nbhd(vec![], vec![zero_len_street]);
        let err = P100PedestrianStreet.apply(&n, "*", &P100Params::defaults(), 0).unwrap_err();
        assert!(
            err.contains("positive real length"),
            "expected 'positive length' error, got: {err}"
        );
    }

    #[test]
    fn all_streets_already_satisfy_target_is_an_error() {
        // 100m street with 4 doors = 4 per 100m, already > 3.0 target.
        let mut buildings = vec![];
        for i in 0..4 {
            buildings.push(building_with_door(&format!("B{i}"), i as f64 * 20.0, 5.0, 10.0));
        }
        let n = nbhd(buildings, vec![ped_street("PED1", 100.0)]);
        let err = P100PedestrianStreet.apply(&n, "*", &P100Params::defaults(), 0).unwrap_err();
        assert!(
            err.contains("already meets"),
            "expected 'street already meets target' error, got: {err}"
        );
    }

    #[test]
    fn no_doorless_adjacent_buildings_is_an_error() {
        // A pedestrian street with no buildings nearby at all.
        let n = nbhd(vec![], vec![ped_street("PED1", 100.0)]);
        let err = P100PedestrianStreet.apply(&n, "*", &P100Params::defaults(), 0).unwrap_err();
        assert!(
            err.contains("no pedestrian street had adjacent doorless buildings"),
            "expected 'no adjacent buildings' error, got: {err}"
        );
    }

    #[test]
    fn happy_path_adds_one_door_to_bring_street_to_target() {
        // 100m street, needs 3 doors (3 per 100m), starts with 0.
        // One doorless building adjacent.
        let buildings = vec![building_without_door("B1", 0.0, 5.0, 10.0)];
        let n = nbhd(buildings, vec![ped_street("PED1", 100.0)]);
        let sub = P100PedestrianStreet
            .apply(&n, "*", &P100Params::defaults(), 0)
            .expect("apply should succeed");

        assert_eq!(sub.new_buildings.len(), 1, "should add one building");
        assert_eq!(
            sub.replaced_building_ids.len(),
            1,
            "should replace one building"
        );

        let modified_b = &sub.new_buildings[0];
        let new_doors: Vec<_> = modified_b
            .openings
            .iter()
            .filter(|o| o.kind == OpeningKind::Door && o.floor == 0 && !o.on_hole)
            .collect();
        assert_eq!(new_doors.len(), 1, "should have exactly 1 new door");
        let door = new_doors[0];
        assert_eq!(door.ring_index, 0, "door should be on ring edge 0");
        assert!((door.t - 0.5).abs() < 1e-6, "door should be at t=0.5 (midpoint)");
        assert!((door.sill_height_m - 0.0).abs() < 1e-6, "door sill should be 0.0");
        assert!((door.head_height_m - 2.1).abs() < 1e-9, "door head should be 2.1");
    }

    #[test]
    fn params_roundtrip() {
        let p = P100Params {
            adjacency_threshold_m: 25.0,
            min_entrances_per_100m: 4.0,
            door_width_m: 1.1,
            door_head_height_m: 2.4,
        };
        let v = p.as_vector();
        let back = P100Params::from_vector(&v);
        assert_eq!(back.adjacency_threshold_m, 25.0);
        assert_eq!(back.min_entrances_per_100m, 4.0);
        assert_eq!(back.door_width_m, 1.1);
        assert_eq!(back.door_head_height_m, 2.4);
    }

    #[test]
    fn one_door_per_building_across_whole_run() {
        // Two pedestrian streets, both needing doors.
        // One building adjacent to both streets. Should only get one door total.
        let street1 = Street {
            id: "PED1".into(),
            centerline: vec![LngLat::new(0.0, -15.0 * m()), LngLat::new(100.0 * m(), -15.0 * m())],
            classification: Some("pedestrian".into()),
            row_width_m: Some(4.0),
            surface: None,
        };
        let street2 = Street {
            id: "PED2".into(),
            centerline: vec![LngLat::new(0.0, 15.0 * m()), LngLat::new(100.0 * m(), 15.0 * m())],
            classification: Some("pedestrian".into()),
            row_width_m: Some(4.0),
            surface: None,
        };
        // Building at y=0, close to both streets.
        let buildings = vec![building_without_door("B1", 0.0, 0.0, 10.0)];
        let n = nbhd(buildings, vec![street1, street2]);
        let sub = P100PedestrianStreet
            .apply(&n, "*", &P100Params::defaults(), 0)
            .expect("apply should succeed");

        assert_eq!(sub.new_buildings.len(), 1, "should modify exactly one building");
        let modified_b = &sub.new_buildings[0];
        let new_doors: Vec<_> = modified_b
            .openings
            .iter()
            .filter(|o| o.kind == OpeningKind::Door && o.floor == 0 && !o.on_hole)
            .collect();
        assert_eq!(
            new_doors.len(),
            1,
            "building should receive exactly 1 door across both streets"
        );
    }

    #[test]
    fn door_width_clamped_to_edge_length() {
        // Very small building (2m x 2m), door_width_m default is 0.9m.
        // Edge is ~2m, so 0.9m fits. But let's test with explicit large param.
        let params = P100Params {
            adjacency_threshold_m: DEFAULT_ADJACENCY_THRESHOLD_M,
            min_entrances_per_100m: DEFAULT_MIN_ENTRANCES_PER_100M,
            door_width_m: 2.0, // Wider than the small building's edge.
            door_head_height_m: DEFAULT_DOOR_HEAD_HEIGHT_M,
        };
        let buildings = vec![building_without_door("B1", 0.0, 5.0, 2.0)];
        let n = nbhd(buildings, vec![ped_street("PED1", 100.0)]);
        let sub = P100PedestrianStreet
            .apply(&n, "*", &params, 0)
            .expect("apply should succeed");

        let modified_b = &sub.new_buildings[0];
        let new_doors: Vec<_> = modified_b
            .openings
            .iter()
            .filter(|o| o.kind == OpeningKind::Door && o.floor == 0 && !o.on_hole)
            .collect();
        assert_eq!(new_doors.len(), 1);
        let door = new_doors[0];
        // Edge is ~2m * 111320 degrees/m ≈ 0.000018 degrees; 2m * 0.9 ≈ 1.8m, clamped to 90% of ~2m.
        // Let's just check it's not larger than the edge length.
        assert!(door.width_m < 2.0, "door width should be clamped to < edge length");
    }
}
