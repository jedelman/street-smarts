//! Neighborhood Intermediate Representation.
//!
//! Single canonical schema all adapters produce and all opinions consume.
//! Decouples scoring from input format.

use crate::geometry::Polygon;
use crate::provenance::ProvenanceTag;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// The top-level neighborhood document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Neighborhood {
    pub id: String,
    pub bbox_wgs84: [f64; 4], // [min_lng, min_lat, max_lng, max_lat]
    #[serde(default)]
    pub parcels: Vec<Parcel>,
    #[serde(default)]
    pub buildings: Vec<Building>,
    #[serde(default)]
    pub streets: Vec<Street>,
    #[serde(default)]
    pub open_space: Vec<OpenSpace>,
    #[serde(default)]
    pub boundaries: Vec<Boundary>,
    #[serde(default)]
    pub activity_nodes: Vec<ActivityNode>,
    pub metadata: NeighborhoodMeta,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeighborhoodMeta {
    pub source: String,
    pub fetched_at: String,
    pub license: String,
    #[serde(default)]
    pub layer_provenance: HashMap<String, ProvenanceTag>,
    /// Human-readable note about what this neighborhood represents
    /// (e.g. "Eastside Commons, current parcel fabric, May 2026").
    pub label: String,
}

/// A parcel = a tax-assessor unit of land. May or may not have a building.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parcel {
    pub id: String,
    pub polygon: Polygon,
    #[serde(default)]
    pub area_acres: f64,
    /// Free-form use category (e.g. "residential", "commercial", "vacant", "civic").
    #[serde(default)]
    pub use_category: Option<String>,
    /// Optional ownership tag — public, private, CLT, cooperative, etc.
    /// This is what feeds the activist `ownership` opinion.
    #[serde(default)]
    pub ownership: Option<Ownership>,
    /// EC-specific: is this part of the Eastside Development Authority (EDA) project?
    #[serde(default)]
    pub is_eda: bool,
    /// Spec code if part of a designed proposal (e.g. "CLT_NORTH", "MAIN_ST_LARGE").
    #[serde(default)]
    pub spec: Option<String>,
    /// Density-ring tier ("core" / "mid" / "edge") set by P29 Density Rings
    /// -- how far this parcel sits from the site's density center. `None`
    /// if P29 hasn't run.
    #[serde(default)]
    pub density_tier: Option<String>,
    /// Target story count. P29 sets it at block scale (a goal for that
    /// block as a whole); P96 Number of Stories overwrites it per-pad with
    /// the actual assigned value once building pads exist. `None` means
    /// no pattern has assigned a target yet -- downstream operators (P107)
    /// fall back to their own flat default.
    #[serde(default)]
    pub target_stories: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Ownership {
    Public,
    Private,
    Clt,        // community land trust
    Cooperative,
    Commons,
    Mixed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Building {
    pub id: String,
    pub polygon: Polygon,
    #[serde(default)]
    pub height_m: Option<f64>,
    #[serde(default)]
    pub typology: Option<String>, // e.g. "tower", "rowhouse", "single-family"
    #[serde(default)]
    pub year_built: Option<i32>,
    #[serde(default)]
    pub parcel_id: Option<String>,
    /// Story count derived from `height_m`. `None` until an operator that
    /// knows the building's floor-to-floor height (e.g.
    /// `p221_natural_doors_and_windows`) has run.
    #[serde(default)]
    pub floors: Option<u32>,
    /// Window/door openings cut into this building's exterior walls.
    /// Empty until a window/door-placing operator has run -- every
    /// existing fixture round-trips unchanged (defaults to `vec![]`).
    #[serde(default)]
    pub openings: Vec<Opening>,
    /// The building's interior, partitioned into cells by
    /// `p127_intimacy_gradient` (depth), `p129_common_areas_at_the_heart`
    /// (which cell is common), and `p131_the_flow_through_rooms`
    /// (adjacency). Empty until those operators have run. Ground-floor
    /// only in this version -- see `InteriorCell.floor`.
    #[serde(default)]
    pub interior_cells: Vec<InteriorCell>,
    /// Real exterior-wall depth in meters, applied uniformly around this
    /// building's own footprint. `None` until an operator that assigns
    /// real wall depth (`p197_thick_walls`) has run -- every existing
    /// fixture round-trips unchanged. Before this field existed, a
    /// `Building`'s outer ring and its `Opening`s modeled a zero-depth
    /// membrane (see `render.py`'s own documented caveat, and
    /// `p197_thick_walls`'s own opinion module doc for the real gap this
    /// closes).
    #[serde(default)]
    pub wall_thickness_m: Option<f64>,
}

/// One cell of a building's interior partition. Deliberately FORM-only --
/// a position in the public/private gradient and a set of connections, not
/// a room type. Nothing here ever says "bedroom" or "kitchen": Alexander's
/// own pattern language describes spatial relationships, not prescribed
/// uses, and this project doesn't assume a program this pipeline has no
/// way to know.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteriorCell {
    pub id: String,
    /// The cell's own 2D footprint.
    pub polygon: Polygon,
    /// Position in the public-to-private gradient: 0.0 sits at the
    /// public-facing wall, 1.0 is the deepest point in the building (a
    /// solid building's far band, or the courtyard-ring point diametrically
    /// opposite the entrance). Set by `p127_intimacy_gradient`.
    pub depth: f64,
    /// True for the one cell `p129_common_areas_at_the_heart` identifies as
    /// nearest the whole footprint's center of gravity.
    #[serde(default)]
    pub is_common: bool,
    /// Free-form, same convention as `Parcel.use_category` /
    /// `Building.typology`: "room" for a normal gradient band/bay, or
    /// "passage" for a P131/P132 loop-closing passage cell.
    #[serde(default)]
    pub kind: String,
    /// Other `InteriorCell` ids this one connects to via an internal
    /// doorway. Set by `p131_the_flow_through_rooms`; empty until it runs.
    #[serde(default)]
    pub connects_to: Vec<String>,
    /// 0 = ground floor. Always 0 in this version -- there's no modeled
    /// way to reach an upper floor (no staircase/vertical circulation
    /// pattern implemented yet), so partitioning one would be fiction.
    #[serde(default)]
    pub floor: u32,
}

/// A window or door opening on one of a `Building`'s exterior walls.
/// Positioned relative to the building's own footprint ring, not in
/// absolute coordinates, so it stays correct if the footprint is
/// reprojected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Opening {
    pub kind: OpeningKind,
    /// Index `i` of the wall edge this opening sits on: the segment from
    /// `ring[i]` to `ring[i+1]` (wrapping), where `ring` is the building
    /// polygon's outer ring, or its (single) hole ring when `on_hole` is
    /// true.
    pub ring_index: usize,
    /// `false` = outer (street/yard-facing) ring. `true` = the courtyard
    /// hole ring of a P107 courtyard-typology building.
    #[serde(default)]
    pub on_hole: bool,
    /// Position of the opening's center along that wall edge, in `0.0..=1.0`.
    pub t: f64,
    pub width_m: f64,
    /// Sill height in meters above this floor's own base elevation (not
    /// above ground) -- add `floor * floor_to_floor_m` to get absolute Z.
    pub sill_height_m: f64,
    pub head_height_m: f64,
    /// 0 = ground floor.
    pub floor: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpeningKind {
    Window,
    Door,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Street {
    pub id: String,
    pub centerline: Vec<crate::geometry::LngLat>,
    #[serde(default)]
    pub classification: Option<String>, // "arterial", "local", "alley", "pedestrian"
    #[serde(default)]
    pub row_width_m: Option<f64>,
    /// Free-form, same convention as `Parcel.use_category` / `Building.typology`:
    /// "grass_pavers" (Alexander's P51 Green Streets -- grass with paving
    /// stones set in for wheels) or "asphalt". `None` means unset (older
    /// fixtures, or a generator that hasn't decided yet) -- not the same
    /// as "asphalt".
    #[serde(default)]
    pub surface: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenSpace {
    pub id: String,
    pub polygon: Polygon,
    pub kind: OpenSpaceKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpenSpaceKind {
    Park,
    Plaza,
    Vacant,
    Sponge,    // stormwater / ecological
    Parking,
    Other,
    /// Land a pattern operator carved out of something else but explicitly
    /// declined to resolve -- distinct from `Vacant` (an assessor's
    /// judgment about existing land) or `Other` (a shrug). This is real
    /// geometry, not a bare number in a trace string: P61's capped-off
    /// candidate squares and dropped slivers land here, and a future P106
    /// (Positive Outdoor Space) check should scan for exactly this kind
    /// rather than trusting operators to have resolved everything.
    Undecided,
    /// Informal shared land within ONE house cluster (P37) -- what the
    /// cluster's households actually face onto and identify with. Distinct
    /// from `Plaza` (P61's intentionally placed, publicly-scaled small
    /// square, or P95's designed interconnecting courtyard): this is
    /// smaller, informal, and belongs to a single cluster rather than the
    /// neighborhood at large.
    Common,
    /// A small, partly enclosed real bump projecting from an adjacent
    /// building's own footprint at a point where it borders a real
    /// `Plaza` -- Alexander's P124 Activity Pockets ("jut forward into
    /// the open space," his own literal words). Distinct from `Plaza`
    /// itself (the square's own open area) and from `Common` (informal
    /// cluster-scale shared land): this is deliberately small, attached
    /// to one specific building, and real geometry added to that
    /// building's own mass, not an independently placed patch.
    Pocket,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Boundary {
    pub id: String,
    pub centerline: Vec<crate::geometry::LngLat>,
    pub kind: BoundaryKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryKind {
    Natural,           // river, ridge, coast
    Infrastructural,   // highway, rail, levee
    Jurisdictional,    // city limit, zoning edge
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityNode {
    pub id: String,
    pub location: crate::geometry::LngLat,
    pub kind: ActivityKind,
    /// Anticipated visits/day or similar intensity hint, if known.
    #[serde(default)]
    pub intensity: Option<f64>,
    #[serde(default)]
    pub label: Option<String>,
    /// Open-ended fit scores, one entry per real activity dimension a
    /// real signal exists for (e.g. "commerce", "gathering", "transit",
    /// "play"), each in `[0.0, 1.0]`. Empty means no fit signal has been
    /// assessed yet -- NOT zero fit for everything. Deliberately not a
    /// fixed enum/classification the way `kind` is: a real place rarely
    /// fits exactly one activity, and this lets a generator or a future
    /// input source express that without committing to a single label.
    #[serde(default)]
    pub activity_fit: BTreeMap<String, f64>,
    /// How public (`1.0`) vs. intimate/private (`0.0`) this node's own
    /// setting reads -- independent of `Street.classification`, so it
    /// doesn't wait on a real "arterial" street existing anywhere in the
    /// generated network. `None` means no real signal exists yet, not a
    /// fabricated midpoint -- see `p126_something_roughly_in_the_middle.rs`
    /// and `p61_small_public_squares.rs`'s own module docs for what
    /// currently sets (or deliberately leaves unset) this field.
    #[serde(default)]
    pub publicness: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    Commerce,
    Civic,
    Transit,
    School,
    Worship,
    Health,
    Other,
}

#[cfg(test)]
mod activity_node_tests {
    use super::*;
    use crate::geometry::LngLat;

    #[test]
    fn old_json_with_no_activity_fit_or_publicness_deserializes_to_honest_defaults() {
        let old_json = r#"{
            "id": "A1",
            "location": {"lng": 0.0, "lat": 0.0},
            "kind": "civic"
        }"#;
        let node: ActivityNode = serde_json::from_str(old_json).expect("older ActivityNode JSON should still deserialize");
        assert!(node.activity_fit.is_empty(), "no fit signal in the source JSON should mean an empty map, not a fabricated one");
        assert_eq!(node.publicness, None, "no publicness signal in the source JSON should mean None, not a fabricated midpoint");
    }

    #[test]
    fn activity_fit_and_publicness_round_trip_through_json() {
        let mut fit = BTreeMap::new();
        fit.insert("commerce".to_string(), 0.7);
        fit.insert("gathering".to_string(), 0.4);
        let node = ActivityNode {
            id: "A1".into(),
            location: LngLat::new(0.0, 0.0),
            kind: ActivityKind::Civic,
            intensity: None,
            label: None,
            activity_fit: fit.clone(),
            publicness: Some(0.8),
        };
        let json = serde_json::to_string(&node).unwrap();
        let back: ActivityNode = serde_json::from_str(&json).unwrap();
        assert_eq!(back.activity_fit, fit);
        assert_eq!(back.publicness, Some(0.8));
    }
}

#[cfg(test)]
mod building_wall_thickness_tests {
    use super::*;
    use crate::geometry::{LngLat, Polygon};

    #[test]
    fn old_json_with_no_wall_thickness_deserializes_to_honest_none() {
        let old_json = r#"{
            "id": "B1",
            "polygon": {"outer": [
                {"lng": 0.0, "lat": 0.0}, {"lng": 0.0001, "lat": 0.0}, {"lng": 0.0001, "lat": 0.0001}
            ]}
        }"#;
        let b: Building = serde_json::from_str(old_json).expect("older Building JSON should still deserialize");
        assert_eq!(b.wall_thickness_m, None, "no thickness signal in the source JSON should mean None, not a fabricated default");
    }

    #[test]
    fn wall_thickness_m_round_trips_through_json() {
        let b = Building {
            id: "B1".into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0), LngLat::new(0.0001, 0.0),
                LngLat::new(0.0001, 0.0001), LngLat::new(0.0, 0.0001), LngLat::new(0.0, 0.0),
            ]),
            height_m: None, typology: None, year_built: None, parcel_id: None,
            floors: None, openings: vec![], interior_cells: vec![],
            wall_thickness_m: Some(0.3),
        };
        let json = serde_json::to_string(&b).unwrap();
        let back: Building = serde_json::from_str(&json).unwrap();
        assert_eq!(back.wall_thickness_m, Some(0.3));
    }
}
