// crates/talaria-sources/src/corpus.rs
//! Provider-agnostic normalized corpus document model.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::kinds::{
    AcademicStatus, AccessLevel, ContributionRole, DocumentType, IdentifierScheme, SourceKind,
};
use crate::types::TypedTimeLite;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedIdentifier {
    pub scheme: IdentifierScheme,
    pub value_raw: String,
    pub value_normalized: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedContribution {
    pub role: ContributionRole,
    pub agent_name: String,
    pub name_normalized: String,
    pub identifier_scheme: Option<IdentifierScheme>,
    pub identifier_value: Option<String>,
    pub ordinal: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedSubject {
    pub scheme: String,
    pub label: String,
    pub identifier: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedCorpusDocument {
    pub source_kind: SourceKind,
    pub external_id: String,
    pub canonical_url: Option<String>,
    pub document_type: DocumentType,
    pub title: String,
    pub language: Option<String>,
    pub abstract_text: Option<String>,
    pub academic_status: AcademicStatus,
    pub access_level: AccessLevel,
    pub full_text_available: bool,
    pub rights_uri: Option<String>,
    pub rights_holder: Option<String>,
    pub rights_normalized: AccessLevel,
    pub publisher_or_institution: Option<String>,
    pub publication_time: TypedTimeLite,
    pub identifiers: Vec<NormalizedIdentifier>,
    pub contributions: Vec<NormalizedContribution>,
    pub subjects: Vec<NormalizedSubject>,
    pub connector_version: String,
    /// Human-readable projection stored when rights allow (no PDF bytes).
    pub snapshot_text: String,
    pub revision_token: Option<String>,
    pub raw_metadata: serde_json::Value,
}

impl NormalizedCorpusDocument {
    /// Stable fingerprint of bibliographic identity + metadata axes.
    /// A remote metadata change that alters any hashed field yields a new snapshot.
    pub fn content_fingerprint(&self) -> String {
        let payload = serde_json::json!({
            "source_kind": self.source_kind.as_str(),
            "external_id": self.external_id,
            "document_type": self.document_type.as_str(),
            "title": self.title,
            "language": self.language,
            "abstract_text": self.abstract_text,
            "academic_status": self.academic_status.as_str(),
            "access_level": self.access_level.as_str(),
            "full_text_available": self.full_text_available,
            "rights_normalized": self.rights_normalized.as_str(),
            "publisher_or_institution": self.publisher_or_institution,
            "publication_time": self.publication_time,
            "identifiers": self.identifiers.iter().map(|i| {
                serde_json::json!([i.scheme.as_str(), i.value_normalized])
            }).collect::<Vec<_>>(),
            "contributions": self.contributions.iter().map(|c| {
                serde_json::json!([
                    c.role.as_str(),
                    c.name_normalized,
                    c.identifier_scheme.map(|s| s.as_str()),
                    c.identifier_value,
                    c.ordinal
                ])
            }).collect::<Vec<_>>(),
            "subjects": self.subjects.iter().map(|s| {
                serde_json::json!([&s.scheme, &s.label, &s.identifier])
            }).collect::<Vec<_>>(),
            "snapshot_text": self.snapshot_text,
        });
        let bytes = serde_json::to_vec(&payload).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        hex::encode(hasher.finalize())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchComponent {
    pub key: String,
    pub weight: f32,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDocumentMatch {
    pub relation: String,
    pub match_version: String,
    pub score: f32,
    pub components: Vec<MatchComponent>,
    pub evidence_summary: String,
}

pub const SUBJECT_MATCH_V1: &str = "subject_match_v1";
