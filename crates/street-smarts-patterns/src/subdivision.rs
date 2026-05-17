//! `PatternOperator` trait and result types.
//!
//! Mirrors the shape of `Opinion`: every operator declares its source citation
//! and produces a transformed neighborhood. The output is one more opinion —
//! the opinion of an algorithm encoding an Alexander pattern.

use serde::{Deserialize, Serialize};
use street_smarts_core::nir::{Neighborhood, Parcel};
use street_smarts_core::opinion::SourceCitation;

/// The result of running a pattern operator: the new parcels (and any new
/// open space) produced, plus a per-feature trace describing why each new
/// feature was created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subdivision {
    /// New parcels that replace the subdivided source parcel(s).
    pub new_parcels: Vec<Parcel>,
    /// Any new open-space features (courtyards, plazas, sponge land) introduced.
    pub new_open_space: Vec<street_smarts_core::nir::OpenSpace>,
    /// IDs of source parcels that this operator transformed (and that should
    /// be REMOVED from the neighborhood when applying).
    pub replaced_parcel_ids: Vec<String>,
    /// Human-readable narrative of what this operator did.
    pub trace: SubdivisionTrace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubdivisionTrace {
    pub operator_name: String,
    pub operator_source: SourceCitation,
    pub headline: String,
    pub steps: Vec<String>,
    /// Echoes of the spec/coalition framing: what this auto-generated proposal
    /// is NOT.
    pub caveats: Vec<String>,
    pub seed: u64,
}

/// Pattern operator protocol. Implementations live alongside this module
/// (e.g. `p95_building_complex.rs`).
pub trait PatternOperator {
    fn name(&self) -> &'static str;
    fn source(&self) -> SourceCitation;
    /// One-line human-readable description.
    fn description(&self) -> &'static str;

    /// Run this operator on a specific parcel of `nbhd`, with the given PRNG
    /// seed. Returns the subdivision result (does not mutate `nbhd`).
    ///
    /// Returns `Err(reason)` if the parcel is not suitable for this operator
    /// (too small, wrong shape, etc.).
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        seed: u64,
    ) -> Result<Subdivision, String>;
}

/// Apply a `Subdivision` to a neighborhood, returning a brand-new
/// `Neighborhood` with the source parcel(s) replaced.
pub fn apply_subdivision(nbhd: &Neighborhood, sub: &Subdivision) -> Neighborhood {
    let removed: std::collections::HashSet<&str> = sub
        .replaced_parcel_ids
        .iter()
        .map(|s| s.as_str())
        .collect();
    let mut new_parcels: Vec<Parcel> = nbhd
        .parcels
        .iter()
        .filter(|p| !removed.contains(p.id.as_str()))
        .cloned()
        .collect();
    new_parcels.extend(sub.new_parcels.iter().cloned());

    let mut new_open_space = nbhd.open_space.clone();
    new_open_space.extend(sub.new_open_space.iter().cloned());

    let mut out = nbhd.clone();
    out.parcels = new_parcels;
    out.open_space = new_open_space;
    // Mark the neighborhood label with operator provenance.
    out.metadata.label = format!(
        "{} — modified by {} (seed {})",
        out.metadata.label, sub.trace.operator_name, sub.trace.seed
    );
    out.id = format!("{}__{}+{}", nbhd.id, sub.trace.operator_name, sub.trace.seed);
    out
}
