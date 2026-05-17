//! Parameters: the trainable substrate of pattern operators.
//!
//! Every `PatternOperator` declares a `Params` associated type. Parameters
//! are the operator's *handles* — bounded floats that the future Mixture-of-
//! Experts router can predict, that the future training loop can optimize,
//! and that a coalition member can tweak by hand right now.
//!
//! Design goals:
//! - **Serializable**: parameters round-trip through JSON for WASM and for
//!   the decision ledger ("the coalition argued for this exact parameter
//!   vector against that one").
//! - **Bounded**: every parameter declares a `(min, max)` and a default. The
//!   router can't propose values outside bounds, and the UI shows sliders.
//! - **Vector-projectable**: `as_vector` and `from_vector` give the bridge
//!   between structured params and the flat representation a NN expects.
//! - **Discoverable**: `schema()` returns named, described parameter specs
//!   so the UI can render them without operator-specific code.
//!
//! Anti-goal: we are NOT building a generic differentiable-programming
//! framework. Operators do hard geometric work that isn't usefully
//! differentiable in places (Voronoi tessellation, ear clipping, polygon
//! intersection). The training loop is expected to use black-box optimization
//! (CMA-ES, evolutionary strategies, or RL via the chorus as reward signal)
//! rather than gradient descent on the operator code directly. Parameters
//! exist to give that loop a clean interface.

use serde::{Deserialize, Serialize};

/// Spec for one parameter — discoverable by the UI / router.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamSpec {
    /// Snake-case identifier (e.g. "target_buildings").
    pub name: String,
    /// One-line description for humans.
    pub description: String,
    /// Inclusive lower bound.
    pub min: f64,
    /// Inclusive upper bound.
    pub max: f64,
    /// Default value (also a sensible starting point for the router).
    pub default: f64,
    /// Optional unit hint for the UI (e.g. "m", "buildings", "ratio").
    #[serde(default)]
    pub unit: Option<String>,
    /// Whether this parameter is INTEGER-valued. Optimizers will round.
    #[serde(default)]
    pub integer: bool,
}

impl ParamSpec {
    pub fn float(name: &str, description: &str, min: f64, max: f64, default: f64) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            min, max, default,
            unit: None,
            integer: false,
        }
    }
    pub fn integer(name: &str, description: &str, min: f64, max: f64, default: f64) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            min, max, default,
            unit: None,
            integer: true,
        }
    }
    pub fn with_unit(mut self, unit: &str) -> Self {
        self.unit = Some(unit.into());
        self
    }
    pub fn clamp(&self, v: f64) -> f64 {
        let v = v.clamp(self.min, self.max);
        if self.integer { v.round() } else { v }
    }
}

/// The protocol every operator's `Params` type implements.
pub trait Parameters: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync {
    /// Discoverable schema for this parameter set.
    fn schema() -> Vec<ParamSpec>;
    /// Default-valued instance (= each ParamSpec.default).
    fn defaults() -> Self;
    /// Project to a flat vector in `schema()` order.
    fn as_vector(&self) -> Vec<f64>;
    /// Build from a flat vector (in `schema()` order); each value clamped
    /// to its spec's bounds. Length mismatches truncate or pad with defaults.
    fn from_vector(v: &[f64]) -> Self;
    /// Convert to a JSON-style map for the decision ledger / UI.
    fn as_map(&self) -> serde_json::Value {
        let schema = Self::schema();
        let vec = self.as_vector();
        let mut map = serde_json::Map::new();
        for (spec, val) in schema.iter().zip(vec.iter()) {
            map.insert(spec.name.clone(), serde_json::json!(val));
        }
        serde_json::Value::Object(map)
    }
}

/// The empty parameter set — for operators that don't have tunables yet.
/// Useful for testing the architecture without a real param set in hand.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NoParams;

impl Parameters for NoParams {
    fn schema() -> Vec<ParamSpec> { vec![] }
    fn defaults() -> Self { Self }
    fn as_vector(&self) -> Vec<f64> { vec![] }
    fn from_vector(_v: &[f64]) -> Self { Self }
}
