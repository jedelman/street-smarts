//! Neighborhood Intermediate Representation.
//!
//! Single canonical schema all adapters produce and all opinions consume.
//! Decouples scoring from input format.

use crate::geometry::Polygon;
use crate::provenance::ProvenanceTag;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The top-level neighborhood document.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

/// A window or door opening on one of a `Building`'s exterior walls.
/// Positioned relative to the building's own footprint ring, not in
/// absolute coordinates, so it stays correct if the footprint is
/// reprojected.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Street {
    pub id: String,
    pub centerline: Vec<crate::geometry::LngLat>,
    #[serde(default)]
    pub classification: Option<String>, // "arterial", "local", "alley", "pedestrian"
    #[serde(default)]
    pub row_width_m: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityNode {
    pub id: String,
    pub location: crate::geometry::LngLat,
    pub kind: ActivityKind,
    /// Anticipated visits/day or similar intensity hint, if known.
    #[serde(default)]
    pub intensity: Option<f64>,
    #[serde(default)]
    pub label: Option<String>,
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
