// crates/talaria-quality/src/lib.rs
//! Quality gate between extraction and canonical events.

mod analyzer;
mod assertion;
mod fingerprint;
mod gates;
mod model;
mod occurrence;
mod projections;
mod resolve;
mod resume;
mod time_typed;

pub use analyzer::{
    split_clauses, ClauseAnalyzeInput, ClauseAnalyzer, ClauseExtraction,
    DeterministicClauseAnalyzer,
};
pub use fingerprint::{candidate_fingerprint, event_fingerprint, normalize_surface};
pub use gates::{apply_gates, GateContext, GateDecision, RejectionCode};
pub use model::{
    CandidateStatus, EntityKind, EventCandidate, EvidencePtr, Mention, ParticipantRole, TypedTime,
    ASSEMBLER_V1, EXTRACTOR_DETERMINISTIC_V1,
};
pub use assertion::{
    competing_places, occurrence_stem_for_event, ABSTAIN_COMPETING_PLACE, EXTRACTOR_EPISTEMIC_STATUS,
};
pub use occurrence::{occurrence_key, occurrence_key_for_event};
pub use projections::{BuildProjections, DerivedLabelProjections, ProjectionEvent};
pub use resolve::{resolve_mentions, GazetteerResolver, MentionResolver, ResolvedMentions};
pub use resume::{
    existing_candidate_action, should_reinforce_existing_event, ExistingCandidateAction,
};
pub use time_typed::{parse_typed_time, typed_time_year};
