//! Print every opinion's value on a neighborhood JSON file, one per line.
//! `cargo run --release -p street-smarts-patterns --example opinions_of -- <nbhd.json>`
//! Diff two runs to see how a pattern operator shifts the chorus.
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::OpinionOutput;
use street_smarts_opinions::evaluate_all;

fn main() {
    let path = std::env::args().nth(1).expect("usage: opinions_of <neighborhood.json>");
    let n: Neighborhood = serde_json::from_str(&std::fs::read_to_string(&path).expect("readable")).expect("parseable");
    let mut rows: Vec<(String, String)> = evaluate_all(&n)
        .into_iter()
        .map(|e| {
            let v = match &e.output {
                OpinionOutput::Value { value, .. } => format!("{value:.3}"),
                OpinionOutput::NoView { .. } => "noview".into(),
                #[allow(unreachable_patterns)]
                _ => "other".into(),
            };
            (e.opinion.name.to_string(), v)
        })
        .collect();
    rows.sort();
    for (name, v) in rows {
        println!("{name} {v}");
    }
}
