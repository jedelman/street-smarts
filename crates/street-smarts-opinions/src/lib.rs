//! # street-smarts-opinions
//!
//! Concrete `Opinion` implementations.
//!
//! v0.1 ships THREE opinions:
//!   - `LevelsOfScale` (geometric, after Alexander/Salingaros 2025)
//!   - `StrongCenters` (geometric, after Alexander/Salingaros 2025)
//!   - `OwnershipPattern` (activist — non-substitutable equity guard)
//!
//! All other geometric opinions, all pattern-presence scorers,
//! and the VLM family are deferred to v0.2.

#![forbid(unsafe_code)]

pub mod geometric;
pub mod activist;
pub mod registry;

pub use registry::{all_opinions_v01, evaluate_all};
