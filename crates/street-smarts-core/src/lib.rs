//! # street-smarts-core
//!
//! The Neighborhood Intermediate Representation (NIR) and supporting types.
//!
//! Per the spec: every output is an *opinion*, never a measurement.
//! There is no `confidence` field by design — opinions are subjective.
//! The only `accurate` thing in this system is the decision ledger.

#![forbid(unsafe_code)]

pub mod geometry;
pub mod nir;
pub mod opinion;
pub mod provenance;

pub use geometry::{LngLat, Polygon, Ring};
pub use nir::{ActivityNode, Boundary, Building, Neighborhood, NeighborhoodMeta, OpenSpace, Parcel, Street};
pub use opinion::{Opinion, OpinionFamily, OpinionOutput, OpinionRef, SourceCitation};
pub use provenance::ProvenanceTag;
