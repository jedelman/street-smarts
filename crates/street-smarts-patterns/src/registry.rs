//! Registry: list available operators and dispatch by name.

use crate::block_grouping::BlockGrouping;
use crate::building_shape::BuildingShape;
use crate::p95_building_complex::P95BuildingComplex;
use crate::p107_wings_of_light::P107WingsOfLight;
use crate::p61_small_public_squares::P61SmallPublicSquares;
use crate::path_network::PathNetwork;
use crate::parameters::ParamSpec;
use crate::subdivision::{DynOperator, Subdivision};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::SourceCitation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorInfo {
    pub name: String,
    pub description: String,
    pub source: SourceCitation,
    pub parameter_schema: Vec<ParamSpec>,
    pub default_params: JsonValue,
}

/// Returns metadata for all v0.1 operators (UI uses this to populate the picker
/// AND render parameter sliders).
pub fn available_operators() -> Vec<OperatorInfo> {
    let ops = all_operators_v01();
    ops.iter()
        .map(|op| OperatorInfo {
            name: op.name().to_string(),
            description: op.description().to_string(),
            source: op.source(),
            parameter_schema: op.parameter_schema(),
            default_params: op.default_params_json(),
        })
        .collect()
}

/// Construct boxed instances of all operators.
///
/// **Order matters**: the typical pipeline runs P95 first (parcel → pads),
/// then BlockGrouping (pads → blocks), then PathNetwork (blocks → streets),
/// then a building-shape pass (pads → buildings) -- either the older
/// BuildingShape stub or P107WingsOfLight, which does real daylight-depth
/// reasoning instead of a plain inscribed rectangle. Both are kept for now
/// so existing pipelines/tests referencing "building_shape" by name keep
/// working; P107 is the one worth reaching for going forward.
pub fn all_operators_v01() -> Vec<Box<dyn DynOperator>> {
    vec![
        // Pipeline order: parcel → pads → blocks → paths → buildings → squares
        Box::new(P95BuildingComplex),
        Box::new(BlockGrouping),
        Box::new(PathNetwork),
        Box::new(P107WingsOfLight),
        Box::new(BuildingShape),
        Box::new(P61SmallPublicSquares),
    ]
}

/// Run a named operator on a parcel. `params_json` is a JSON object (named)
/// or array (vector form) of parameter values; pass `Value::Null` for defaults.
pub fn run_operator(
    nbhd: &Neighborhood,
    operator_name: &str,
    parcel_id: &str,
    params_json: &JsonValue,
    seed: u64,
) -> Result<Subdivision, String> {
    let ops = all_operators_v01();
    let op = ops
        .iter()
        .find(|o| o.name() == operator_name)
        .ok_or_else(|| format!("unknown operator: {operator_name}"))?;
    op.apply_json(nbhd, parcel_id, params_json, seed)
}
