//! The chorus on a neighborhood, slimmed for the walk to voice: every
//! opinion's name, family, source, value (or why it abstains), its own
//! one-line finding, and the ids of the features that drove it. The walk's
//! passers-by say these out loud (public/sparrow/chorus.js); nothing in a
//! line is theirs but the opener.
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
            json!({
                "name": op["name"],
                "family": op["family"],
                "source": op["source"]["display"],
                "url": op["source"]["url"],
                "value": value,
                "says": says,
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
