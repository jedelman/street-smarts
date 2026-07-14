//! P95 Building Complex — pattern-presence opinion.
//!
//! From Alexander, *A Pattern Language*, Pattern 95:
//! > Never build large monolithic buildings. Whenever possible translate
//! > your building program into a building complex, whose parts manifest
//! > the actual social facts of the situation.
//!
//! This is the pattern's OWN opinion of itself — unlike `levels_of_scale`
//! or `strong_centers`, which see building-complex output only as a side
//! effect of general area-hierarchy or center-distribution properties, this
//! opinion looks directly for the thing P95 claims to produce: multiple
//! discrete building pads clustered around an interconnecting courtyard,
//! not one dominant blob.
//!
//! Detection is structural, keyed to how `P95BuildingComplex::apply` names
//! its output (`{parcel_id}_P95_cell_{n}` parcels, `{parcel_id}_P95_courtyard_p{n}`
//! open space). This opinion is therefore coupled to that operator's naming
//! convention — a real coupling, not hidden. If the operator's naming
//! changes, this breaks loudly (NoView), not silently.
//!
//! Score components, per detected complex:
//! - `presence`: 0.0 at 1 pad (monolithic — the exact anti-pattern), scaling
//!   to 1.0 at 4+ pads. This is deliberately the harshest component: P95's
//!   entire point is plurality of parts, so a single pad is scored as a
//!   near-total miss, not a partial credit.
//! - `courtyard`: 1.0 if the complex has an interconnecting courtyard, 0.0
//!   if not (a set of pads with no shared open space isn't a complex, it's
//!   just several buildings).
//! - `balance`: 1.0 minus the coefficient of variation of pad areas, clamped.
//!   Penalizes "one giant pad + several slivers" — a degenerate way to
//!   satisfy `presence` and `courtyard` without satisfying the pattern's
//!   intent that parts "manifest the actual social facts of the situation."

use std::collections::BTreeMap;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P95BuildingComplexOpinion;

struct Complex {
    pad_ids: Vec<String>,
    pad_areas_m2: Vec<f64>,
    has_courtyard: bool,
}

impl Opinion for P95BuildingComplexOpinion {
    fn name(&self) -> &'static str { "p95_building_complex" }
    fn family(&self) -> OpinionFamily { OpinionFamily::Pattern }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p95".into(),
            display: "Alexander et al., A Pattern Language, Pattern 95 (Building Complex)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl95/apl95.htm".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) { (0.0, 1.0) }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        // Group P95-generated pads by source parcel id, using the
        // operator's own naming convention: "{parcel_id}_P95_cell_{n}".
        let mut complexes: BTreeMap<String, Complex> = BTreeMap::new();

        for p in &n.parcels {
            if let Some((prefix, _)) = p.id.split_once("_P95_cell_") {
                let entry = complexes.entry(prefix.to_string()).or_insert_with(|| Complex {
                    pad_ids: Vec::new(),
                    pad_areas_m2: Vec::new(),
                    has_courtyard: false,
                });
                entry.pad_ids.push(p.id.clone());
                let area = if p.area_acres > 0.0 { p.area_acres * 4046.86 } else { p.polygon.area_m2() };
                entry.pad_areas_m2.push(area);
            }
        }

        for o in &n.open_space {
            if let Some((prefix, _)) = o.id.split_once("_P95_courtyard_p") {
                if let Some(entry) = complexes.get_mut(prefix) {
                    entry.has_courtyard = true;
                }
            }
        }

        if complexes.is_empty() {
            return OpinionOutput::NoView {
                reason: "No P95-generated building complexes found (no parcels matching \
                         the '_P95_cell_' naming convention)."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut per_complex_values: Vec<f64> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();
        let mut contributing_features: Vec<String> = Vec::new();
        let mut headline_parts: Vec<String> = Vec::new();

        for (id, complex) in &complexes {
            let n_pads = complex.pad_ids.len();
            let presence = ((n_pads as f64 - 1.0) / 3.0).clamp(0.0, 1.0);
            let courtyard = if complex.has_courtyard { 1.0 } else { 0.0 };

            let balance = if n_pads >= 2 {
                let mean = complex.pad_areas_m2.iter().sum::<f64>() / n_pads as f64;
                if mean > 0.0 {
                    let variance = complex.pad_areas_m2.iter()
                        .map(|a| (a - mean).powi(2))
                        .sum::<f64>() / n_pads as f64;
                    let cv = variance.sqrt() / mean;
                    (1.0 - cv).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            } else {
                // A single pad has no size distribution to be balanced. Its
                // failure is entirely captured by `presence`; don't double
                // penalize by also zeroing `balance`.
                1.0
            };

            let value = 0.5 * presence + 0.25 * courtyard + 0.25 * balance;
            per_complex_values.push(value);

            details.insert(format!("{}.n_pads", id), n_pads.to_string());
            details.insert(format!("{}.has_courtyard", id), complex.has_courtyard.to_string());
            details.insert(format!("{}.presence", id), format!("{:.3}", presence));
            details.insert(format!("{}.courtyard_score", id), format!("{:.3}", courtyard));
            details.insert(format!("{}.balance", id), format!("{:.3}", balance));
            details.insert(format!("{}.value", id), format!("{:.3}", value));

            contributing_features.extend(complex.pad_ids.clone());

            headline_parts.push(format!(
                "{}: {} pad{}{}, score {:.2}",
                id, n_pads, if n_pads == 1 { "" } else { "s" },
                if complex.has_courtyard { "" } else { " (no courtyard)" },
                value
            ));
        }

        let value = per_complex_values.iter().sum::<f64>() / per_complex_values.len() as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("mean_presence".into(), per_complex_values.iter().sum::<f64>() / per_complex_values.len() as f64);

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} P95 complex{} found. {}",
                complexes.len(),
                if complexes.len() == 1 { "" } else { "es" },
                headline_parts.join("; ")
            ),
            sub_scores,
            details,
            caveats: vec![
                "Detection is keyed to P95BuildingComplex's own output naming convention \
                 ('_P95_cell_', '_P95_courtyard_p'). Complexes produced by hand or by a \
                 differently-named operator won't be seen.".into(),
                "Scores building-COUNT and courtyard presence, not architectural quality — \
                 doesn't see facade, massing, or whether pads plausibly hold real buildings.".into(),
                "Pad count here can come from non-convex boundary clipping fragmentation, not \
                 just from min_buildings/max_buildings. A single Voronoi seed can shatter into \
                 many disjoint pads on a concave parcel. This opinion cannot currently tell \
                 deliberate decomposition apart from incidental fragmentation.".into(),
            ],
            contributing_features,
            runtime_ms: timer.elapsed_ms(),
        }
    }
}
