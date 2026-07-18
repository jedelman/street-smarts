//! Provenance tracking. Every data layer carries its source.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProvenanceTag {
    pub source: String,
    pub fetched_at: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}
