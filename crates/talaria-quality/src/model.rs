// crates/talaria-quality/src/model.rs
//! Strict EventCandidate model and related value types.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const EXTRACTOR_DETERMINISTIC_V1: &str = "deterministic:clause_v1";
pub const ASSEMBLER_V1: &str = "assemble:v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Person,
    Place,
    Object,
    Organization,
    Unknown,
}

impl EntityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Place => "place",
            Self::Object => "object",
            Self::Organization => "organization",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "person" => Self::Person,
            "place" => Self::Place,
            "object" => Self::Object,
            "organization" => Self::Organization,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantRole {
    Spouse,
    Participant,
    Opponent,
    Ally,
    Other,
}

impl ParticipantRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spouse => "spouse",
            Self::Participant => "participant",
            Self::Opponent => "opponent",
            Self::Ally => "ally",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateStatus {
    Pending,
    NeedsReview,
    Accepted,
    Rejected,
    Assembled,
}

impl CandidateStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::NeedsReview => "needs_review",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Assembled => "assembled",
        }
    }
}

/// Typed time — never a bare string as source of truth.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypedTime {
    Exact {
        year: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        month: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        day: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        surface: Option<String>,
    },
    Range {
        start_year: i32,
        end_year: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        surface: Option<String>,
    },
    Approx {
        year: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        surface: Option<String>,
    },
    Unknown {
        #[serde(skip_serializing_if = "Option::is_none")]
        surface: Option<String>,
    },
}

impl TypedTime {
    pub fn year_for_gates(&self) -> Option<i32> {
        match self {
            Self::Exact { year, .. } | Self::Approx { year, .. } => Some(*year),
            Self::Range { start_year, .. } => Some(*start_year),
            Self::Unknown { .. } => None,
        }
    }

    pub fn canonical_key(&self) -> String {
        match self {
            Self::Exact {
                year, month, day, ..
            } => {
                format!("exact:{year}:{}:{}", month.unwrap_or(0), day.unwrap_or(0))
            }
            Self::Range {
                start_year,
                end_year,
                ..
            } => format!("range:{start_year}:{end_year}"),
            Self::Approx { year, .. } => format!("approx:{year}"),
            Self::Unknown { .. } => "unknown".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mention {
    pub surface: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<EntityKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<ParticipantRole>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidencePtr {
    pub fragment_id: Uuid,
    pub clause_index: i32,
    pub start_offset: i32,
    pub end_offset: i32,
    pub quoted_text: String,
}

/// In-memory EventCandidate (mirrors DB row; title is NOT source of truth).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventCandidate {
    pub id: Uuid,
    pub snapshot_id: Uuid,
    pub fragment_id: Uuid,
    pub clause_index: i32,

    pub subject_surface: String,
    pub subject_entity_id: Option<Uuid>,

    pub event_type: String,
    pub predicate: String,
    pub time: TypedTime,

    pub place_mentions: Vec<Mention>,
    pub object_mentions: Vec<Mention>,
    pub participant_mentions: Vec<Mention>,

    pub place_entity_id: Option<Uuid>,
    pub place_label: Option<String>,

    pub evidence_ptrs: Vec<EvidencePtr>,
    pub extractor_version: String,
    pub fingerprint: String,

    pub status: CandidateStatus,
    pub rejection_codes: Vec<String>,
}

impl EventCandidate {
    /// Invariant: place_entity_id may only be set when resolved place kind is Place.
    pub fn assert_place_kind_invariant(&self, place_kind: Option<EntityKind>) -> bool {
        match self.place_entity_id {
            None => true,
            Some(_) => place_kind == Some(EntityKind::Place),
        }
    }
}
