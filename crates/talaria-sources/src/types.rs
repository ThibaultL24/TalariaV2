// crates/talaria-sources/src/types.rs
use serde::{Deserialize, Serialize};

use crate::kinds::{DiscoveryMethod, DocumentType, SourceKind};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalEntityRef {
    pub system: String,
    pub id: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMetadata {
    pub raw: serde_json::Value,
}

impl Default for SourceMetadata {
    fn default() -> Self {
        Self {
            raw: serde_json::json!({}),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypedTimeLite {
    Exact { year: i32, surface: Option<String> },
    Unknown { surface: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredDocument {
    pub source_kind: SourceKind,
    pub external_id: String,
    pub canonical_url: Option<String>,
    pub title: String,
    pub language: Option<String>,
    pub document_type: DocumentType,
    pub subject_links: Vec<ExternalEntityRef>,
    pub publication_time: Option<TypedTimeLite>,
    pub discovery_method: DiscoveryMethod,
    pub relevance_score: f32,
    pub source_metadata: SourceMetadata,
}
