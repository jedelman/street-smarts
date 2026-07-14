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
