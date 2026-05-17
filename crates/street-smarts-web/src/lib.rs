//! # street-smarts-web
//!
//! WASM bindings: takes a JSON `Neighborhood`, returns a JSON `DisagreementReport`.
//! Also exposes pattern operators for subdivision.

use wasm_bindgen::prelude::*;

/// Initialize panic-hook so Rust panics surface as console errors.
#[wasm_bindgen(start)]
pub fn _start() {
    console_error_panic_hook::set_once();
}

/// Evaluate all v0.1 opinions against a neighborhood JSON string.
/// Returns a JSON string of `DisagreementReport`.
#[wasm_bindgen]
pub fn analyze_neighborhood(neighborhood_json: &str) -> Result<String, JsValue> {
    let nbhd: street_smarts_core::nir::Neighborhood = serde_json::from_str(neighborhood_json)
        .map_err(|e| JsValue::from_str(&format!("parse neighborhood: {e}")))?;
    let evaluated = street_smarts_opinions::evaluate_all(&nbhd);
    let report = street_smarts_conflict::build_report(evaluated);
    serde_json::to_string(&report)
        .map_err(|e| JsValue::from_str(&format!("serialize report: {e}")))
}

/// List available pattern operators as a JSON array. Each entry has
/// `name`, `description`, and a `source` citation.
#[wasm_bindgen]
pub fn list_operators() -> Result<String, JsValue> {
    let ops = street_smarts_patterns::available_operators();
    serde_json::to_string(&ops)
        .map_err(|e| JsValue::from_str(&format!("serialize operators: {e}")))
}

/// Apply a pattern operator to a parcel inside the given neighborhood JSON.
/// `params_json` is a JSON string of either an object (named params) or
/// an array (vector form). Pass `"null"` or an empty string for defaults.
/// Returns a JSON object: `{ "neighborhood": ..., "trace": ... }`.
#[wasm_bindgen]
pub fn subdivide_parcel(
    neighborhood_json: &str,
    parcel_id: &str,
    operator_name: &str,
    params_json: &str,
    seed: u64,
) -> Result<String, JsValue> {
    let nbhd: street_smarts_core::nir::Neighborhood = serde_json::from_str(neighborhood_json)
        .map_err(|e| JsValue::from_str(&format!("parse neighborhood: {e}")))?;
    let params: serde_json::Value = if params_json.is_empty() || params_json == "null" {
        serde_json::Value::Null
    } else {
        serde_json::from_str(params_json)
            .map_err(|e| JsValue::from_str(&format!("parse params: {e}")))?
    };
    let sub = street_smarts_patterns::run_operator(&nbhd, operator_name, parcel_id, &params, seed)
        .map_err(|e| JsValue::from_str(&format!("operator: {e}")))?;
    let modified = street_smarts_patterns::apply_subdivision(&nbhd, &sub);
    let response = serde_json::json!({
        "neighborhood": modified,
        "trace": sub.trace,
    });
    serde_json::to_string(&response)
        .map_err(|e| JsValue::from_str(&format!("serialize result: {e}")))
}

/// Library version string for the UI footer.
#[wasm_bindgen]
pub fn version() -> String {
    format!("street-smarts v{}", env!("CARGO_PKG_VERSION"))
}
