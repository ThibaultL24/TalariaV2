// crates/talaria-quality/src/lib.rs
//! Quality gate between extraction and canonical events.

mod analyzer;
mod fingerprint;
mod gates;
mod model;
mod occurrence;
mod projections;
mod resolve;
mod time_typed;

pub use analyzer::{
    split_clauses, ClauseAnalyzeInput, ClauseAnalyzer, ClauseExtraction, DeterministicClauseAnalyzer,
};
pub use fingerprint::{candidate_fingerprint, event_fingerprint, normalize_surface};
pub use gates::{apply_gates, GateContext, GateDecision, RejectionCode};
pub use model::{
    CandidateStatus, EntityKind, EventCandidate, EvidencePtr, Mention, ParticipantRole, TypedTime,
    ASSEMBLER_V1, EXTRACTOR_DETERMINISTIC_V1,
};
pub use occurrence::occurrence_key;
pub use projections::{BuildProjections, DerivedLabelProjections, ProjectionEvent};
pub use resolve::{resolve_mentions, GazetteerResolver, MentionResolver, ResolvedMentions};
pub use time_typed::{parse_typed_time, typed_time_year};
