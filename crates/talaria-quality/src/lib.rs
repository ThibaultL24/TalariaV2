// crates/talaria-quality/src/lib.rs
//! Livrable 1: semantic quality gate between extraction and canonical events.
//!
//! Path: ClauseAnalyzer → EventCandidate → typed resolution → gates → assemble.
//! No fake COSMOS adapter. Legacy `phrase_candidates` path remains untouched.

mod analyzer;
mod fingerprint;
mod gates;
mod model;
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
    EXTRACTOR_DETERMINISTIC_V1, ASSEMBLER_V1,
};
pub use projections::{BuildProjections, DerivedLabelProjections, ProjectionEvent};
pub use resolve::{resolve_mentions, GazetteerResolver, MentionResolver, ResolvedMentions};
pub use time_typed::{parse_typed_time, typed_time_year};
