//! Content-addressed history for Neighborhood states -- the foundation
//! the decision ledger ("the record of what was decided") builds on. See
//! PRIMITIVES_SPEC.md §2 and IMPLEMENTATION_PLAN.md Phase 4.

pub mod corrected_pipeline;
pub mod history;

pub use corrected_pipeline::run_corrected_pipeline_via_ledger;
pub use history::{
    block_membership, hash_neighborhood, Commit, HistoryStore, InMemoryHistoryStore, NeighborhoodId,
};
