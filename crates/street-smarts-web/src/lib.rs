//! # street-smarts-web
//!
//! WASM bindings: takes a JSON `Neighborhood`, returns a JSON `DisagreementReport`.
//! The browser loads this WASM module, fetches the fixtures, and renders.

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

/// Library version string for the UI footer.
#[wasm_bindgen]
pub fn version() -> String {
    format!("street-smarts v{}", env!("CARGO_PKG_VERSION"))
}
