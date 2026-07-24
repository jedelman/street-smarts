//! # street-smarts-core
//!
//! The Neighborhood Intermediate Representation (NIR) and supporting types.
//!
//! Per the spec: every output is an *opinion*, never a measurement.
//! There is no `confidence` field by design — opinions are subjective.
//! The only `accurate` thing in this system is the decision ledger.

#![forbid(unsafe_code)]

pub mod components;
pub mod geometry;
pub mod nir;
pub mod opinion;
pub mod provenance;
pub mod scope;
pub mod sdf;
pub mod timer;
pub mod world;

pub use components::{ring_tier_label, DensityTier};
pub use geometry::{LngLat, Polygon, PolygonPart, Ring};
pub use nir::{ActivityNode, Boundary, Building, Neighborhood, NeighborhoodMeta, OpenSpace, Parcel, Street};
pub use opinion::{Capability, Opinion, OpinionFamily, OpinionOutput, OpinionRef, SourceCitation};
pub use provenance::ProvenanceTag;
pub use scope::Scope;
pub use sdf::{sdf_box, sdf_cylinder, sdf_difference, sdf_intersection, sdf_smin, sdf_sphere, sdf_union, AABB3D, Vec3};
pub use timer::Timer;
pub use world::World;
