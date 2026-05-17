//! Activist opinions — equity axes.
//! v0.1: Ownership pattern.
//!
//! These are CATEGORICAL guards in the generator, not aggregable axes.
//! In the conflict engine, the activist family stands apart from the geometric
//! family — disagreement between them never gets averaged.

pub mod ownership;

pub use ownership::OwnershipPattern;
