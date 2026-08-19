// crates/talaria-intuition/src/lib.rs
//! Canonical debate bundles for Intuition. No DB, no RPC.

mod canon;
mod fact;
mod plan;

pub use canon::{
    atom_record, build_debate_bundle, fingerprint_hex, full_slug, normalize_slug_fragment,
    parse_full_slug, predicate_atom, situated_context_triples, triple_record, AtomRecord,
    CanonError, DebateBundle, TripleRecord, SCHEMA_VERSION, VOCAB_ATOM_KINDS,
};
pub use fact::{
    category_term, event_atom_name, event_same_as, fact_fingerprint, fact_from_place_conflict,
    fact_from_soft_claim, start_date_field, AboutEvent, DebateFact, DebateText, SCHEMA_VERSION_V2,
};
pub use plan::{
    debate_from_place_conflict, debate_from_soft_claim, ConflictGroup, PlannedDebate,
    SoftClaimInput,
};
