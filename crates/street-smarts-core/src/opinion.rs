//! The `Opinion` protocol — every scorer in the system implements this.
//!
//! Critical design notes from the spec:
//! - Every opinion declares its `SourceCitation`. The library never speaks in its own voice.
//! - There is NO `confidence` field. Opinions either have a view (`OpinionOutput::Value`)
//!   or explicitly decline (`OpinionOutput::NoView`). No false precision.
//! - The "I don't have a view here" output is first-class.

use crate::nir::Neighborhood;
use serde::{Deserialize, Serialize};

/// What family an opinion belongs to. Lets the conflict engine cluster outputs.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum OpinionFamily {
    /// Classical geometric algorithms scoring an Alexander property.
    Geometric,
    /// Pattern presence/quality/coverage scorers (P29, P30, etc.).
    Pattern,
    /// Equity / activist axes — categorical guards, not aggregable.
    Activist,
    /// Vision-language model running Salingaros prompts.
    /// Not used in v0.1.
    Vlm,
    /// A human tap/click/vote captured into the ledger.
    Human,
}

/// Where this opinion's logic comes from. Every output cites its source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceCitation {
    /// Short identifier, e.g. "salingaros_2025", "alexander_apl_p30", "jason_edelman".
    pub id: String,
    /// Human-readable attribution.
    pub display: String,
    /// Optional URL or DOI.
    #[serde(default)]
    pub url: Option<String>,
}

/// A reference to an opinion (for the conflict engine's records, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpinionRef {
    pub name: String,
    pub family: OpinionFamily,
    pub source: SourceCitation,
}

/// The output of running an opinion against a neighborhood.
///
/// `Value` and `NoView` are equally first-class. An opinion that returns
/// `NoView` is saying "I don't have a view here" — not "I failed."
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OpinionOutput {
    Value {
        /// A number in this opinion's declared `value_range`.
        value: f64,
        /// One-line summary suitable for showing a human reader.
        method_summary: String,
        /// Named sub-scores that combine into `value`. Lets the UI show the math
        /// without committing every opinion to the same shape. Keys are
        /// opinion-specific (e.g. "presence", "hierarchy", "distribution").
        #[serde(default)]
        sub_scores: std::collections::BTreeMap<String, f64>,
        /// Free-form numeric details that aren't 0–1 scores: counts, ratios,
        /// distances. UI shows these as a key/value table.
        #[serde(default)]
        details: std::collections::BTreeMap<String, String>,
        /// What this opinion explicitly *doesn't* see. Drives disagreement framing.
        #[serde(default)]
        caveats: Vec<String>,
        /// IDs of features (parcel.id, building.id, etc.) that drove the value.
        #[serde(default)]
        contributing_features: Vec<String>,
        /// How long the opinion took to evaluate, milliseconds.
        runtime_ms: u32,
        /// Pinned model identifier for opinions backed by a frontier model
        /// (the VLM family, once it exists) -- e.g. "claude-opus-4-8-2026xxxx".
        /// `None` for every non-VLM opinion; reserved now, ahead of that
        /// family being built, so a drift-calibration check (comparing this
        /// same opinion's score on a fixed reference site over time) has
        /// something to key on from day one instead of a schema retrofit
        /// later. See HARDENING_SPEC.md #6.
        #[serde(default)]
        model_version: Option<String>,
    },
    NoView {
        reason: String,
        runtime_ms: u32,
    },
}

impl OpinionOutput {
    pub fn value(&self) -> Option<f64> {
        match self {
            OpinionOutput::Value { value, .. } => Some(*value),
            OpinionOutput::NoView { .. } => None,
        }
    }
}

/// The `Opinion` trait. Implementors live in `street-smarts-opinions`.
///
/// Not a single global trait object — each opinion family has its own implementations
/// and the `evaluate_all` machinery in `street-smarts-opinions` dispatches.
pub trait Opinion {
    fn name(&self) -> &'static str;
    fn family(&self) -> OpinionFamily;
    fn source(&self) -> SourceCitation;
    fn value_range(&self) -> (f64, f64);

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput;

    fn as_ref(&self) -> OpinionRef {
        OpinionRef {
            name: self.name().to_string(),
            family: self.family(),
            source: self.source(),
        }
    }
}
