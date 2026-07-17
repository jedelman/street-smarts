//! # street-smarts-opinions
//!
//! Concrete `Opinion` implementations.
//!
//! v0.1 shipped four opinions; v0.2/v0.3 add three more:
//!   - `LevelsOfScale` (geometric, after Alexander/Salingaros 2025)
//!   - `StrongCenters` (geometric, after Alexander/Salingaros 2025)
//!   - `OwnershipPattern` (activist — non-substitutable equity guard)
//!   - `P95BuildingComplexOpinion` (pattern — scores P95's own output directly)
//!   - `P106PositiveOutdoorSpace` (pattern — convexity + resolved-land check
//!     over ALL open space, not just P95/P61's)
//!   - `P21FourStoryLimit` (pattern — area-weighted ordinary-height
//!     compliance + spacing of tall exceptions, checking what
//!     `street-smarts-patterns::p96_number_of_stories` actually produced)
//!
//! All other geometric opinions, remaining pattern-presence scorers,
//! and the VLM family are deferred to a later version.

#![forbid(unsafe_code)]

pub mod geometric;
pub mod activist;
pub mod pattern;
pub mod registry;
#[cfg(feature = "vlm")]
pub mod vlm;

pub use registry::{all_opinions_v01, evaluate_all};
