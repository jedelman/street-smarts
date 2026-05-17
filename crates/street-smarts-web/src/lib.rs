//! # street-smarts-web
//!
//! WASM bindings for the browser. Single entry point: take a Neighborhood
//! as JSON, return a DisagreementReport as JSON.
//!
//! Runs entirely in the browser. No network, no API calls. The activist
//! path stays cloud-free per the spec's hard constraint.

#![forbid(unsafe_code)]

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Evaluate all v0.1 opinions against a neighborhood JSON and return a
/// DisagreementReport JSON. Returns an error string if input parse fails.
#[wasm_bindgen]
pub fn evaluate_neighborhood(neighborhood_json: &str) -> Result<String, JsError> {
    let nbhd: street_smarts_core::nir::Neighborhood = serde_json::from_str(neighborhood_json)
        .map_err(|e| JsError::new(&format!("failed to parse neighborhood JSON: {}", e)))?;

    let evaluated = street_smarts_opinions::evaluate_all(&nbhd);
    let report = street_smarts_conflict::build_report(evaluated);

    serde_json::to_string(&report)
        .map_err(|e| JsError::new(&format!("failed to serialize report: {}", e)))
}

/// Return library version and v0.1 opinion roster as JSON.
#[wasm_bindgen]
pub fn library_info() -> String {
    let opinions = street_smarts_opinions::all_opinions_v01();
    let info: Vec<_> = opinions
        .iter()
        .map(|op| {
            serde_json::json!({
                "name": op.name(),
                "family": format!("{:?}", op.family()).to_lowercase(),
                "source": op.source(),
                "value_range": op.value_range(),
            })
        })
        .collect();
    serde_json::json!({
        "library": "street-smarts",
        "version": env!("CARGO_PKG_VERSION"),
        "v": "0.1",
        "opinions": info,
        "tagline": "A provocation engine for neighborhood imagination. The library speaks as a chorus of cited opinions, never in its own voice.",
    })
    .to_string()
}
