//! `PatternOperator` trait and result types.
//!
//! ## Design: operators as trainable modules
//!
//! Every operator declares an associated `Params` type — a serializable,
//! bounded, vector-projectable parameter set. This gives:
//!
//! - **Tunable handles for the UI now**: sliders per parameter, with bounds.
//! - **A target for the future training loop**: a MoE router predicts an
//!   operator and a parameter vector; the parameter vector is projected
//!   back to the structured form via `Parameters::from_vector`.
//! - **A serialization boundary for the decision ledger**: parameter sets
//!   round-trip through JSON, so coalition arguments can cite the exact
//!   parameters that produced a specific proposal.
//!
//! We expose operators in TWO forms:
//! - `PatternOperator<P>` — the typed form, generic in its `Params`. Use
//!   directly in Rust code that knows which operator it's calling.
//! - `dyn DynOperator` — the type-erased form, takes/returns `serde_json::Value`
//!   for params. Use in registries, dispatchers, and the WASM boundary.
//!
//! Both shapes live side-by-side; each operator's `impl` block satisfies
//! both via a blanket bridge.

use crate::parameters::{ParamSpec, Parameters};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use street_smarts_core::nir::{Neighborhood, Parcel};
use street_smarts_core::opinion::SourceCitation;

/// The result of running a pattern operator: the new parcels, open space,
/// buildings, and/or streets produced, plus a per-feature trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subdivision {
    /// New parcels that replace the subdivided source parcel(s).
    #[serde(default)]
    pub new_parcels: Vec<Parcel>,
    /// New open-space features (courtyards, plazas, sponge land).
    #[serde(default)]
    pub new_open_space: Vec<street_smarts_core::nir::OpenSpace>,
    /// New buildings. Operators at the parcel level (P95) leave this empty;
    /// the BuildingShape operator populates it.
    #[serde(default)]
    pub new_buildings: Vec<street_smarts_core::nir::Building>,
    /// New streets / path segments. The PathNetwork operator populates this.
    #[serde(default)]
    pub new_streets: Vec<street_smarts_core::nir::Street>,
    /// IDs of source parcels removed from the neighborhood when applying.
    #[serde(default)]
    pub replaced_parcel_ids: Vec<String>,
    /// Human-readable narrative.
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
    /// The actual parameter values used. Coalition can cite these in ledger.
    pub params: JsonValue,
}

/// Typed operator protocol. Each operator has a concrete `Params` type.
pub trait PatternOperator {
    type Params: Parameters;

    fn name(&self) -> &'static str;
    fn source(&self) -> SourceCitation;
    /// One-line human-readable description.
    fn description(&self) -> &'static str;

    /// Schema for this operator's parameters — delegates to `Params::schema`
    /// by default but can be overridden if an operator wants to filter or
    /// reorder its public surface.
    fn parameter_schema(&self) -> Vec<ParamSpec> {
        Self::Params::schema()
    }

    /// Default parameter values.
    fn default_params(&self) -> Self::Params {
        Self::Params::defaults()
    }

    /// Run this operator with explicit parameters.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String>;
}

/// Type-erased operator protocol. Implemented automatically for every
/// typed `PatternOperator` via the blanket impl below.
pub trait DynOperator: Send + Sync {
    fn name(&self) -> &'static str;
    fn source(&self) -> SourceCitation;
    fn description(&self) -> &'static str;
    fn parameter_schema(&self) -> Vec<ParamSpec>;
    fn default_params_json(&self) -> JsonValue;
    fn apply_json(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params_json: &JsonValue,
        seed: u64,
    ) -> Result<Subdivision, String>;
}

impl<T> DynOperator for T
where
    T: PatternOperator + Send + Sync,
{
    fn name(&self) -> &'static str { <T as PatternOperator>::name(self) }
    fn source(&self) -> SourceCitation { <T as PatternOperator>::source(self) }
    fn description(&self) -> &'static str { <T as PatternOperator>::description(self) }

    fn parameter_schema(&self) -> Vec<ParamSpec> {
        <T as PatternOperator>::parameter_schema(self)
    }

    fn default_params_json(&self) -> JsonValue {
        let p = <T as PatternOperator>::default_params(self);
        p.as_map()
    }

    fn apply_json(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params_json: &JsonValue,
        seed: u64,
    ) -> Result<Subdivision, String> {
        // Resolve params: if null or empty, use defaults; else project through
        // the parameter schema.
        let params = if params_json.is_null() {
            <T as PatternOperator>::default_params(self)
        } else {
            // Accept either a flat array (vector form) or an object (named form).
            let schema = <T as PatternOperator>::parameter_schema(self);
            let mut vec = vec![0.0; schema.len()];
            for (i, spec) in schema.iter().enumerate() {
                vec[i] = spec.default;
            }
            if let Some(arr) = params_json.as_array() {
                for (i, v) in arr.iter().enumerate().take(vec.len()) {
                    if let Some(f) = v.as_f64() { vec[i] = f; }
                }
            } else if let Some(obj) = params_json.as_object() {
                for (i, spec) in schema.iter().enumerate() {
                    if let Some(v) = obj.get(&spec.name).and_then(|x| x.as_f64()) {
                        vec[i] = v;
                    }
                }
            } else {
                return Err("params must be a JSON object or array".into());
            }
            // Clamp each.
            for (i, spec) in schema.iter().enumerate() {
                if let Some(v) = vec.get_mut(i) {
                    *v = spec.clamp(*v);
                }
            }
            <T as PatternOperator>::Params::from_vector(&vec)
        };
        <T as PatternOperator>::apply(self, nbhd, parcel_id, &params, seed)
    }
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

    let mut new_buildings = nbhd.buildings.clone();
    new_buildings.extend(sub.new_buildings.iter().cloned());

    let mut new_streets = nbhd.streets.clone();
    new_streets.extend(sub.new_streets.iter().cloned());

    let mut out = nbhd.clone();
    out.parcels = new_parcels;
    out.open_space = new_open_space;
    out.buildings = new_buildings;
    out.streets = new_streets;
    out.metadata.label = format!(
        "{} — modified by {} (seed {})",
        out.metadata.label, sub.trace.operator_name, sub.trace.seed
    );
    out.id = format!("{}__{}+{}", nbhd.id, sub.trace.operator_name, sub.trace.seed);
    out
}
