//! Registry: list available operators and dispatch by name.

use crate::p95_building_complex::P95BuildingComplex;
use crate::subdivision::{PatternOperator, Subdivision};
use serde::{Deserialize, Serialize};
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::SourceCitation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorInfo {
    pub name: String,
    pub description: String,
    pub source: SourceCitation,
}

/// Returns metadata for all v0.1 operators (UI uses this to populate the picker).
pub fn available_operators() -> Vec<OperatorInfo> {
    let ops = all_operators_v01();
    ops.iter()
        .map(|op| OperatorInfo {
            name: op.name().to_string(),
            description: op.description().to_string(),
            source: op.source(),
        })
        .collect()
}

/// Construct boxed instances of all operators.
pub fn all_operators_v01() -> Vec<Box<dyn PatternOperator>> {
    vec![Box::new(P95BuildingComplex)]
}

/// Run a named operator on a parcel. Returns the Subdivision (does not mutate
/// the input neighborhood; caller applies via `apply_subdivision`).
pub fn run_operator(
    nbhd: &Neighborhood,
    operator_name: &str,
    parcel_id: &str,
    seed: u64,
) -> Result<Subdivision, String> {
    let ops = all_operators_v01();
    let op = ops
        .iter()
        .find(|o| o.name() == operator_name)
        .ok_or_else(|| format!("unknown operator: {operator_name}"))?;
    op.apply(nbhd, parcel_id, seed)
}
