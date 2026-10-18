// crates/talaria-sources/src/snac/mod.rs
//! SNAC identity decisions. HTTP parsing waits until captured payloads are bound.
//!
//! Captured read shape (2026-10-18), not a guessed schema:
//! `constellation` is one object; `constellation.ark`, `id`, `version`;
//! `constellation.sameAsRelations[].uri`; `constellation.relations`;
//! `constellation.resourceRelations[].resource.documentType.term` and `.link`.
//! The git fixtures are reduced extracts. Full-body SHA-256 values are in
//! `tests/fixtures/snac/original-sha256.txt`.

pub mod align;
pub mod ark;
pub mod assertion;

pub use align::{decide_candidates, CandidateView, LinkDecision, LinkStatus, SubjectAuthorities};
pub use ark::{
    canonical_ark, normalize_isni, normalize_qid, normalize_viaf, parse_same_as, SameAs,
};
pub use assertion::{
    assertion_content_hash, canonical_assertion_json, map_predicate, source_assertion_key,
    AssertionParts, MappedPredicate, ASSERTION_KEY_VERSION,
};
