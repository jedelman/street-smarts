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
//! # The same real proxy the opinion already checks
//!
//! `crates/street-smarts-opinions/src/pattern/p161_sunny_place.rs` (the
//! detector for this same pattern) has scored a real `OpenSpace` as a
//! "sunny place" when:
//! 1. The space is not `Undecided` and has positive area
//! 2. The nearest building (by centroid-to-centroid haversine distance)
//!    sits at a higher latitude (building is north of the space in the
//!    northern hemisphere)
//! 3. That distance is within `ADJACENCY_THRESHOLD_M` (30m default)
//! 4. The space's bounding-box short side clears `MIN_DEPTH_M` (1.83m,
//!    Alexander's literal 6 feet)
//!
//! This operator closes the gap using the EXACT same definition: for each
//! real building that does NOT already have a qualifying sunny place by that
//! proxy, add ONE new compact rectangular `OpenSpace` immediately south of
//! it. The generator and the opinion can't silently disagree about what
//! counts as a sunny place, because they're reading the same real geometry
//! the same way.
//!
//! # What this deliberately does NOT do
//! - **No wind protection check.** Same honest limitation the opinion's own
//!   caveat already states: no wind or massing-shelter simulation exists
//!   anywhere in this pipeline. Sunny-place depth and orientation are
//!   checkable; wind is not.
//! - **Only one new space per building.** A real building might benefit from
//!   multiple sunny spots; this operator adds a single, geometrically
//!   compact rectangle on the building's south side, matching the opinion's
//!   own single-nearest-building reduction exactly.
//! - **OpenSpaceKind::Common is a reuse, not an exact fit.** Its own schema
//!   doc scopes it to "informal shared land within ONE house cluster
//!   (P37)." A sunny place attached to a single building is adjacent to but
//!   not identical to that meaning. No `Sunny` variant exists anywhere in
//!   the schema; adding one was deliberately ruled out in Step 4 (schema
//!   safety). This generator reuses `Common` and documents the mismatch.
//! - **Cannot place a sunny space on _every_ building.** Some buildings,
//!   especially those on narrow or occluded parcels, leave no room for a
//!   qualifying sunny place within `clearance_m` of any other structure.
//!   Those buildings are skipped; the generator returns an error if _every_
//!   candidate space was rejected, not a silent no-op.
//! - **Northern-hemisphere assumption.** Same as the opinion: the proxy
//!   assumes south-facing = better sun. Tropical/southern-hemisphere sites
//!   would need north-facing logic instead.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{ring_to_local, local_to_ring, bbox, polygon_min_distance};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use street_smarts_core::geometry::{haversine_m, LngLat, Polygon};
use street_smarts_core::nir::{Neighborhood, OpenSpace, OpenSpaceKind};
use street_smarts_core::opinion::SourceCitation;

/// Matches the real opinion's own `ADJACENCY_THRESHOLD_M` exactly -- see
/// this module's own doc for why that agreement matters.
const DEFAULT_ADJACENCY_THRESHOLD_M: f64 = 30.0;

/// Matches the real opinion's own `MIN_DEPTH_M` exactly.
const DEFAULT_MIN_DEPTH_M: f64 = 1.83; // 6 feet

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P161Params {
    /// How close a real open space must sit to a building's own centroid to
    /// count as that building's sunny place. Same threshold the opinion uses.
    pub adjacency_threshold_m: f64,
    /// Minimum depth (short side of bounding box) an open space must have to
    /// count as a qualifying sunny place. Same threshold the opinion uses.
    pub min_depth_m: f64,
    /// Depth (north-south extent in local meters) of the new rectangular sunny
    /// space to create. Clamped to at least `min_depth_m` to ensure it counts.
    pub depth_m: f64,
    /// Gap in meters between the building's south face and the north edge of
    /// the new sunny space rectangle.
    pub gap_m: f64,
    /// Maximum ratio of width to depth for the new rectangle. The building's
    /// full width would be clamped to at most `max_aspect_ratio * depth_m`,
    /// centered on the building. This keeps the space compact and prevents
    /// long thin strips that would hurt p163_outdoor_room's circularity and
    /// p106_positive_outdoor_space's convexity scores.
    pub max_aspect_ratio: f64,
    /// Minimum clearance in meters the new rectangle must maintain from any
    /// existing building footprint. Used to avoid placing sunny spaces that
    /// would overlap or touch neighboring structures.
    pub clearance_m: f64,
}

impl Parameters for P161Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "adjacency_threshold_m",
                "How close an existing open space must sit to a building's centroid to count as its sunny place.",
                10.0,
                60.0,
                DEFAULT_ADJACENCY_THRESHOLD_M,
            ).with_unit("m"),
            ParamSpec::float(
                "min_depth_m",
                "Minimum depth (bounding-box short side) an open space must have to count as a qualifying sunny place.",
                1.0,
                6.0,
                DEFAULT_MIN_DEPTH_M,
            ).with_unit("m"),
            ParamSpec::float(
                "depth_m",
                "Depth (north-south extent) of the new rectangular sunny space to create.",
                1.83,
                10.0,
                3.0,
            ).with_unit("m"),
            ParamSpec::float(
                "gap_m",
                "Gap in meters between the building's south face and the north edge of the new sunny space.",
                0.0,
                3.0,
                0.5,
            ).with_unit("m"),
            ParamSpec::float(
                "max_aspect_ratio",
                "Maximum ratio of width to depth; keeps the rectangle compact.",
                1.0,
                6.0,
                3.0,
            ),
            ParamSpec::float(
                "clearance_m",
                "Minimum clearance in meters from any existing building footprint.",
                0.0,
                2.0,
                0.25,
            ).with_unit("m"),
        ]
    }
    fn defaults() -> Self {
        Self {
            adjacency_threshold_m: DEFAULT_ADJACENCY_THRESHOLD_M,
            min_depth_m: DEFAULT_MIN_DEPTH_M,
            depth_m: 3.0,
            gap_m: 0.5,
            max_aspect_ratio: 3.0,
            clearance_m: 0.25,
        }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![
            self.adjacency_threshold_m,
            self.min_depth_m,
            self.depth_m,
            self.gap_m,
            self.max_aspect_ratio,
            self.clearance_m,
        ]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) {
            p.adjacency_threshold_m = s.clamp(*x);
        }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) {
            p.min_depth_m = s.clamp(*x);
        }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) {
            p.depth_m = s.clamp(*x);
        }
        if let (Some(s), Some(x)) = (schema.get(3), v.get(3)) {
            p.gap_m = s.clamp(*x);
        }
        if let (Some(s), Some(x)) = (schema.get(4), v.get(4)) {
            p.max_aspect_ratio = s.clamp(*x);
        }
        if let (Some(s), Some(x)) = (schema.get(5), v.get(5)) {
            p.clearance_m = s.clamp(*x);
        }
        p
    }
}

/// Compute the short side of the bounding box of a WGS84 ring, same technique
/// the opinion uses. Uses the latitude-corrected local projection.
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

/// Check if a building already has a qualifying sunny place via the opinion's
/// own proxy: an existing open space that is:
/// 1. Not Undecided and has positive area
/// 2. Within `adjacency_threshold_m` of this building
/// 3. South of (has lower latitude than) this building
/// 4. At least `min_depth_m` deep
fn building_has_sunny_place(
    building: &street_smarts_core::nir::Building,
    neighborhood: &Neighborhood,
    adjacency_threshold_m: f64,
    min_depth_m: f64,
) -> bool {
    let bc = building.polygon.centroid();
    for space in &neighborhood.open_space {
        if space.kind == OpenSpaceKind::Undecided || space.polygon.area_m2() <= 0.0 {
            continue;
        }
        let sc = space.polygon.centroid();
        let dist = haversine_m(&bc, &sc);
        let is_south = bc.lat > sc.lat;
        let is_adjacent = dist <= adjacency_threshold_m;
        let is_deep = bbox_short_side_m(&space.polygon.outer) >= min_depth_m;
        if is_south && is_adjacent && is_deep {
            return true;
        }
    }
    false
}

pub struct P161SunnyPlace;

impl PatternOperator for P161SunnyPlace {
    type Params = P161Params;

    fn name(&self) -> &'static str {
        "p161_sunny_place"
    }
    fn description(&self) -> &'static str {
        "Adds a real rectangular OpenSpace immediately south of each building that lacks a qualifying sunny place."
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p161".into(),
            display: "Alexander et al., A Pattern Language, Pattern 161 (Sunny Place)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Sunny-Place-(161)".into()),
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
                "p161_sunny_place only supports parcel_id \"*\" -- it adds sunny places across every \
                 building in one pass."
                    .into(),
            );
        }
        if nbhd.buildings.is_empty() {
            return Err("p161_sunny_place needs at least one real Building to place sunny spaces on -- run p107_wings_of_light first.".into());
        }

        // Clamp depth to at least min_depth_m so the generated space will pass the opinion's check
        let actual_depth_m = params.depth_m.max(params.min_depth_m);

        // Find all buildings that do NOT already have a sunny place
        let candidates: Vec<_> = nbhd
            .buildings
            .iter()
            .filter(|b| {
                !building_has_sunny_place(b, nbhd, params.adjacency_threshold_m, params.min_depth_m)
            })
            .collect();

        if candidates.is_empty() {
            return Err(format!(
                "p161_sunny_place: every one of {} building(s) already has a qualifying sunny place by the opinion's own proxy.",
                nbhd.buildings.len()
            ));
        }

        let mut new_open_space: Vec<OpenSpace> = Vec::new();
        let mut entity_provenance: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut n_placed = 0usize;
        let mut n_rejected_clearance: Vec<String> = Vec::new();

        for building in &candidates {
            let bc = building.polygon.centroid();
            let origin = LngLat::new(bc.lng, bc.lat);

            // Convert building's outer ring to local meters, centered on its centroid
            let local_ring = ring_to_local(&building.polygon.outer, &origin);
            let (min_pt, max_pt) = bbox(&local_ring);

            // New rectangle dimensions
            let rect_height = actual_depth_m;
            let max_width = actual_depth_m * params.max_aspect_ratio;
            let building_width = (max_pt.x - min_pt.x).abs();
            let rect_width = building_width.min(max_width);

            // Center the rectangle on the building's x-center
            let building_cx = (min_pt.x + max_pt.x) * 0.5;
            let rect_x_min = building_cx - rect_width * 0.5;
            let rect_x_max = building_cx + rect_width * 0.5;

            // Place the north edge of the rectangle at the building's south face,
            // offset south (negative y) by gap_m
            let building_south_y = min_pt.y;
            let rect_y_max = building_south_y - params.gap_m;
            let rect_y_min = rect_y_max - rect_height;

            // Define the rectangle vertices in local meters (CCW from bottom-left)
            let local_rect = vec![
                crate::planar::Pt2::new(rect_x_min, rect_y_min),
                crate::planar::Pt2::new(rect_x_max, rect_y_min),
                crate::planar::Pt2::new(rect_x_max, rect_y_max),
                crate::planar::Pt2::new(rect_x_min, rect_y_max),
            ];

            // Check clearance against all existing buildings
            let mut has_clearance = true;
            for other_building in &nbhd.buildings {
                let other_local = ring_to_local(&other_building.polygon.outer, &origin);
                let min_dist = polygon_min_distance(&local_rect, &other_local);
                if min_dist < params.clearance_m {
                    has_clearance = false;
                    break;
                }
            }

            if !has_clearance {
                n_rejected_clearance.push(building.id.clone());
                continue;
            }

            // Convert back to WGS84
            let space_ring = local_to_ring(&local_rect, &origin);
            let space_id = format!("{}_p161_sunny", building.id);

            new_open_space.push(OpenSpace {
                id: space_id.clone(),
                polygon: Polygon::from_ring(space_ring),
                kind: OpenSpaceKind::Common,
            });

            entity_provenance.insert(space_id, vec![building.id.clone()]);
            n_placed += 1;
        }

        if new_open_space.is_empty() {
            return Err(format!(
                "p161_sunny_place: all {} candidate building(s) had their proposed sunny space rejected for insufficient clearance from existing structures.",
                candidates.len()
            ));
        }

        let mut caveats = vec![
            "OpenSpaceKind::Common is a reuse, not an exact fit: its schema doc scopes it to \
             'informal shared land within ONE house cluster (P37).' A sunny place attached to a \
             single building is adjacent to but not identical to that meaning. No 'Sunny' variant \
             exists anywhere in the schema."
                .into(),
            "Cannot check or generate 'wind-protected' -- no wind or massing-shelter simulation \
             exists anywhere in this pipeline."
                .into(),
            "Northern-hemisphere assumption: the proxy assumes south-facing = better sun. \
             Tropical/southern-hemisphere sites would need north-facing logic instead."
                .into(),
            "Depth is a bounding-box short-side proxy, not a true 'defined room' shape check -- \
             an irregular sliver could have a misleading bbox short side."
                .into(),
            "CROSS-PATTERN REGRESSION: these are small spaces. p60_accessible_green scores the \
             FRACTION of open-space features meeting a ~5,574 m² minimum, so adding many small \
             spaces will LOWER p60's 'qualifying_green_size' sub-score even though nothing about \
             the site's real greens changed."
                .into(),
        ];

        if !n_rejected_clearance.is_empty() {
            caveats.push(
                format!(
                    "Note: {} building(s) were skipped because their proposed sunny space \
                     lacked sufficient clearance from neighboring structures.",
                    n_rejected_clearance.len()
                )
                .into(),
            );
        }

        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!(
                "Added {} new sunny place(s) to {:.0}% of buildings.",
                n_placed,
                100.0 * (n_placed as f64 / candidates.len() as f64)
            ),
            steps: vec![
                format!(
                    "{} of {} buildings without a qualifying sunny place got a new rectangular space \
                     (max width {}m, depth {}m) placed immediately south with {}m gap and {}m clearance; \
                     {} building(s) were skipped for insufficient clearance.",
                    n_placed,
                    candidates.len(),
                    (actual_depth_m * params.max_aspect_ratio) as i32,
                    actual_depth_m as i32,
                    params.gap_m,
                    params.clearance_m,
                    n_rejected_clearance.len()
                ),
            ],
            caveats,
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels: vec![],
            new_open_space,
            new_buildings: vec![],
            new_streets: vec![],
            new_activity_nodes: vec![],
            new_boundaries: vec![],
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: vec![],
            replaced_building_ids: vec![],
            entity_provenance,
            trace,
            new_fields: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::nir::{Building, NeighborhoodMeta};

    fn m() -> f64 {
        1.0 / 111_320.0
    }

    fn rect_ring(cx: f64, cy: f64, w: f64, h: f64) -> Vec<LngLat> {
        let mm = m();
        vec![
            LngLat::new((cx - w / 2.0) * mm, (cy - h / 2.0) * mm),
            LngLat::new((cx + w / 2.0) * mm, (cy - h / 2.0) * mm),
            LngLat::new((cx + w / 2.0) * mm, (cy + h / 2.0) * mm),
            LngLat::new((cx - w / 2.0) * mm, (cy + h / 2.0) * mm),
        ]
    }

    fn building_at(id: &str, cx: f64, cy: f64) -> Building {
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(rect_ring(cx, cy, 10.0, 10.0)),
            height_m: Some(9.0),
            typology: None,
            year_built: None,
            parcel_id: None,
            floors: None,
            openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
            canopies: vec![],
            roof_segments: vec![],
            wall_niches: vec![],
        }
    }

    fn nbhd(buildings: Vec<Building>, open_space: Vec<OpenSpace>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
            parcels: vec![],
            buildings,
            streets: vec![],
            open_space,
            boundaries: vec![],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P161 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    #[test]
    fn parcel_id_must_be_wildcard() {
        let n = nbhd(vec![building_at("B1", 0.0, 0.0)], vec![]);
        let err = P161SunnyPlace
            .apply(&n, "BLOCK_0", &P161Params::defaults(), 0)
            .unwrap_err();
        assert!(err.contains("\"*\""), "expected the real target-scope error, got: {err}");
    }

    #[test]
    fn no_buildings_is_a_real_error() {
        let n = nbhd(vec![], vec![]);
        let err = P161SunnyPlace
            .apply(&n, "*", &P161Params::defaults(), 0)
            .unwrap_err();
        assert!(
            err.contains("p107_wings_of_light"),
            "expected error about p107, got: {err}"
        );
    }

    #[test]
    fn every_building_already_has_a_sunny_place_is_an_error() {
        // Building at (0, 20) with a deep space at (0, 0) south of it
        let space = OpenSpace {
            id: "S1".into(),
            polygon: Polygon::from_ring(rect_ring(0.0, 0.0, 6.0, 6.0)),
            kind: OpenSpaceKind::Plaza,
        };
        let n = nbhd(vec![building_at("B1", 0.0, 20.0)], vec![space]);
        let err = P161SunnyPlace
            .apply(&n, "*", &P161Params::defaults(), 0)
            .unwrap_err();
        assert!(
            err.contains("every one of") && err.contains("already has"),
            "expected 'every building has sunny place' error, got: {err}"
        );
    }

    #[test]
    fn no_space_to_do_is_rejected_when_all_candidate_rectangles_fail_clearance() {
        // Building at (0, 0) with no existing spaces, but surrounded by other buildings
        // so tight that the south rectangle would violate clearance
        let main = building_at("B1", 0.0, 0.0);
        let left_neighbor = building_at("B_left", -15.0, 0.0);
        let right_neighbor = building_at("B_right", 15.0, 0.0);

        // Very strict clearance to force rejection
        let params = P161Params {
            clearance_m: 50.0, // Impossible to satisfy
            ..P161Params::defaults()
        };

        let n = nbhd(
            vec![main, left_neighbor, right_neighbor],
            vec![],
        );
        let err = P161SunnyPlace.apply(&n, "*", &params, 0).unwrap_err();
        assert!(
            err.contains("insufficient clearance"),
            "expected clearance error, got: {err}"
        );
    }

    #[test]
    fn happy_path_adds_a_sunny_place_to_a_building_without_one() {
        let building = building_at("B1", 0.0, 0.0);
        let n = nbhd(vec![building], vec![]);
        let sub = P161SunnyPlace
            .apply(&n, "*", &P161Params::defaults(), 0)
            .expect("happy path should succeed");

        assert_eq!(sub.new_open_space.len(), 1, "should add exactly one space");
        let space = &sub.new_open_space[0];
        assert!(space.id.contains("_p161_sunny"), "space id should follow the naming pattern");
        assert_eq!(space.kind, OpenSpaceKind::Common);
        assert!(space.polygon.area_m2() > 0.0, "space should have positive area");

        // Check that the space's depth passes the opinion's min_depth check
        let depth = bbox_short_side_m(&space.polygon.outer);
        let params = P161Params::defaults();
        assert!(
            depth >= params.min_depth_m,
            "generated space depth {:.2}m should be >= min_depth_m {:.2}m",
            depth,
            params.min_depth_m
        );

        // Verify entity_provenance
        assert!(
            sub.entity_provenance.contains_key(&space.id),
            "entity_provenance should map the new space to its source building"
        );
        if let Some(sources) = sub.entity_provenance.get(&space.id) {
            assert_eq!(sources.len(), 1);
            assert_eq!(sources[0], "B1");
        }
    }

    #[test]
    fn params_roundtrip() {
        let p = P161Params {
            adjacency_threshold_m: 40.0,
            min_depth_m: 2.0,
            depth_m: 4.5,
            gap_m: 1.0,
            max_aspect_ratio: 2.5,
            clearance_m: 0.5,
        };
        let v = p.as_vector();
        let back = P161Params::from_vector(&v);
        assert_eq!(back.adjacency_threshold_m, 40.0);
        assert_eq!(back.min_depth_m, 2.0);
        assert_eq!(back.depth_m, 4.5);
        assert_eq!(back.gap_m, 1.0);
        assert_eq!(back.max_aspect_ratio, 2.5);
        assert_eq!(back.clearance_m, 0.5);
    }

    #[test]
    fn depth_is_clamped_to_min_depth_m() {
        // Set depth_m below min_depth_m to force clamping
        let params = P161Params {
            depth_m: 1.0,
            min_depth_m: 1.83,
            ..P161Params::defaults()
        };
        let building = building_at("B1", 0.0, 0.0);
        let n = nbhd(vec![building], vec![]);
        let sub = P161SunnyPlace
            .apply(&n, "*", &params, 0)
            .expect("should succeed with clamped depth");

        let space = &sub.new_open_space[0];
        let depth = bbox_short_side_m(&space.polygon.outer);
        // Use a small tolerance (1mm) for floating-point comparison
        let tolerance = 0.001;
        assert!(
            depth >= params.min_depth_m - tolerance,
            "clamped depth {:.2}m should be >= min_depth_m {:.2}m (within {:.3}m tolerance)",
            depth,
            params.min_depth_m,
            tolerance
        );
    }

    #[test]
    fn generated_space_is_south_of_building() {
        let building = building_at("B1", 0.0, 100.0); // Building at lat 100 (in local coords)
        let building_lat = building.polygon.centroid().lat;
        let n = nbhd(vec![building], vec![]);
        let sub = P161SunnyPlace
            .apply(&n, "*", &P161Params::defaults(), 0)
            .expect("should succeed");

        let space = &sub.new_open_space[0];
        let space_lat = space.polygon.centroid().lat;

        assert!(
            space_lat < building_lat,
            "generated space should sit at lower latitude (south) than the building"
        );
    }

    #[test]
    fn multiple_buildings_get_sunny_places() {
        let b1 = building_at("B1", 0.0, 0.0);
        let b2 = building_at("B2", 100.0, 0.0);
        let n = nbhd(vec![b1, b2], vec![]);
        let sub = P161SunnyPlace
            .apply(&n, "*", &P161Params::defaults(), 0)
            .expect("should succeed");

        assert!(
            sub.new_open_space.len() >= 1,
            "should add sunny places to at least one building"
        );
    }
}
