//! The chorus on a neighborhood, slimmed for the walk to voice: every
//! opinion's name, family, source, value (or why it abstains), its own
//! one-line finding, its counts and measures, and the ids of the features
//! that drove it. The walk's passers-by say these out loud in plain words
//! (public/sparrow/plain.js), using only the numbers here.
//!
//! `cargo run --release -p street-smarts-conflict --example chorus_of -- <nbhd.json> > chorus.json`
use serde_json::{json, Value};
use street_smarts_core::nir::Neighborhood;

/// A feature id as the GLB's element table names it: "b.window[3]" and
/// "b.cell_5" are about building b; "a~b" and "a<->b" are about both.
fn feature_ids(raw: &str) -> Vec<String> {
    raw.split("<->")
        .flat_map(|s| s.split('~'))
        .map(|s| s.split('.').next().unwrap_or(s).trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: chorus_of <neighborhood.json>");
    let n: Neighborhood = serde_json::from_str(&std::fs::read_to_string(&path).expect("readable")).expect("parseable");
    let report = street_smarts_conflict::build_report(street_smarts_opinions::evaluate_all(&n));
    let report = serde_json::to_value(&report).expect("serializable");

    let opinions: Vec<Value> = report["opinions"]
        .as_array()
        .expect("opinions")
        .iter()
        .map(|e| {
            let (op, out) = (&e["opinion"], &e["output"]);
            let mut features: Vec<String> = Vec::new();
            for f in out["contributing_features"].as_array().into_iter().flatten() {
                for id in feature_ids(f.as_str().unwrap_or_default()) {
                    if !features.contains(&id) && features.len() < 300 {
                        features.push(id);
                    }
                }
            }
            let value = out.get("value").cloned().unwrap_or(Value::Null);
            let says = if value.is_null() { &out["reason"] } else { &out["method_summary"] };
            // The opinion's own counts and measures (the walk's plain-speech lines
            // quote them), minus the per-feature entries ("b.end_has_goal").
            let mut details = serde_json::Map::new();
            for (k, v) in out["details"].as_object().into_iter().flatten() {
                if !k.contains('.') && v.as_str().is_some_and(|s| s.len() < 40) {
                    details.insert(k.clone(), v.clone());
                }
            }
            json!({
                "name": op["name"],
                "family": op["family"],
                "source": op["source"]["display"],
                "url": op["source"]["url"],
                "value": value,
                "says": says,
                "details": details,
                "features": features,
            })
        })
        .collect();
    let questions: Vec<Value> = report["questions_for_humans"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|q| json!({ "question": q["question"], "why": q["why_it_matters"] }))
        .collect();
    println!("{}", json!({ "version": 1, "opinions": opinions, "questions": questions }));
}
