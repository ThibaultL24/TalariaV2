// crates/talaria-quality/src/lib.rs
//! Quality gate between extraction and canonical events.

mod analyzer;
mod attribution;
mod assertion;
mod fingerprint;
mod gates;
mod grounding;
mod model;
mod occurrence;
mod places;
mod overlay;
mod projections;
mod resolve;
mod resume;
mod time_typed;

pub use attribution::{
    attribution_gate_decision, auto_accept_attribution, classify_attribution, AttributionInput,
    AttributionMatch,
};
pub use analyzer::{
    split_clauses, ClauseAnalyzeInput, ClauseAnalyzer, ClauseExtraction, CosmosClauseAnalyzer,
    CosmosJudgment, CosmosTuple, DeterministicClauseAnalyzer, HeuristicCosmosAnalyzer,
    COSMOS_DEFAULT_MIN_SCORE, COSMOS_HEURISTIC_ID, COSMOS_HEURISTIC_V1,
};
pub use fingerprint::{candidate_fingerprint, event_fingerprint, normalize_surface};
pub use gates::{apply_gates, event_implies_subject_presence, event_type_is_map_locus, GateContext, GateDecision, RejectionCode};
pub use grounding::{
    accept_items, agent_is_other_person, parse_lane, quote_is_grounded, validate_item, GroundedItem,
    Lane, RawExtractItem, RejectReason,
};
pub use model::{
    CandidateStatus, EntityKind, EventCandidate, EvidencePtr, Mention, ParticipantRole, TypedTime,
    ASSEMBLER_V1, EXTRACTOR_DETERMINISTIC_V1,
};
pub use assertion::{
    competing_places, occurrence_stem_for_event, ABSTAIN_COMPETING_PLACE, EXTRACTOR_EPISTEMIC_STATUS,
};
pub use occurrence::{occurrence_key, occurrence_key_for_event};
pub use places::{place_query, PlaceQuery};
pub use overlay::{
    overlay_effect, parse_overlay_verdicts, OverlayEffect, OverlayItem, OverlayVerdict,
};
pub use projections::{BuildProjections, DerivedLabelProjections, ProjectionEvent};
pub use resolve::{
    gazetteer_place_in_text, resolve_mentions, GazetteerResolver, MentionResolver, ResolvedMentions,
};
pub use resume::{
    existing_candidate_action, should_reinforce_existing_event, ExistingCandidateAction,
};
pub use time_typed::{
    extract_time_surface, parse_typed_time, start_time_from_typed, time_to_json, typed_time_year,
};
