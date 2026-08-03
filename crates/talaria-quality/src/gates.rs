// crates/talaria-quality/src/gates.rs
//! Deterministic quality gates — no subject-specific hardcoding.

use crate::model::{CandidateStatus, EntityKind, EventCandidate, TypedTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RejectionCode {
    CrossClauseJoin,
    InvalidPlaceKind,
    EventBeforeSubjectBirth,
    EventAfterSubjectDeath,
    ImplausibleAgeForEventType,
    SingletonCardinalityViolation,
    MissingEvidence,
    DuplicateCandidate,
}

impl RejectionCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CrossClauseJoin => "cross_clause_join",
            Self::InvalidPlaceKind => "invalid_place_kind",
            Self::EventBeforeSubjectBirth => "event_before_subject_birth",
            Self::EventAfterSubjectDeath => "event_after_subject_death",
            Self::ImplausibleAgeForEventType => "implausible_age_for_event_type",
            Self::SingletonCardinalityViolation => "singleton_cardinality_violation",
            Self::MissingEvidence => "missing_evidence",
            Self::DuplicateCandidate => "duplicate_candidate",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "cross_clause_join" => Self::CrossClauseJoin,
            "invalid_place_kind" => Self::InvalidPlaceKind,
            "event_before_subject_birth" => Self::EventBeforeSubjectBirth,
            "event_after_subject_death" => Self::EventAfterSubjectDeath,
            "implausible_age_for_event_type" => Self::ImplausibleAgeForEventType,
            "singleton_cardinality_violation" => Self::SingletonCardinalityViolation,
            "missing_evidence" => Self::MissingEvidence,
            "duplicate_candidate" => Self::DuplicateCandidate,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct GateContext {
    /// Known birth year of subject (from prior accepted quality birth, if any).
    pub subject_birth_year: Option<i32>,
    /// Known death year of subject.
    pub subject_death_year: Option<i32>,
    /// True if an active quality birth/death already exists for this subject.
    pub has_active_birth: bool,
    pub has_active_death: bool,
    /// True when fingerprint already maps to an assembled/accepted candidate.
    pub fingerprint_exists: bool,
    /// Signals were joined across clause boundaries during extraction.
    pub cross_clause_join_detected: bool,
    /// Kind of entity assigned to place_entity_id (if any).
    pub place_entity_kind: Option<EntityKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateDecision {
    Accept,
    NeedsReview,
    Reject(Vec<RejectionCode>),
}

impl GateDecision {
    pub fn status(&self) -> CandidateStatus {
        match self {
            Self::Accept => CandidateStatus::Accepted,
            Self::NeedsReview => CandidateStatus::NeedsReview,
            Self::Reject(_) => CandidateStatus::Rejected,
        }
    }

    pub fn codes(&self) -> Vec<String> {
        match self {
            Self::Reject(codes) => codes.iter().map(|c| c.as_str().to_string()).collect(),
            _ => vec![],
        }
    }
}

/// Generic age plausibility by event type (not subject-specific).
fn implausible_age(event_type: &str, age: i32) -> bool {
    match event_type {
        "birth" => false,
        "death" => age < 0 || age > 120,
        "marriage" | "divorce" => age < 12 || age > 100,
        "battle" | "office" | "diplomatic" | "employment" | "exile" | "imprisonment" => {
            age < 10 || age > 100
        }
        "education" => age < 3 || age > 90,
        _ => age < 0 || age > 120,
    }
}

pub fn apply_gates(candidate: &EventCandidate, ctx: &GateContext) -> GateDecision {
    let mut rejects = Vec::new();

    if ctx.cross_clause_join_detected {
        rejects.push(RejectionCode::CrossClauseJoin);
    }

    if candidate.evidence_ptrs.is_empty() {
        rejects.push(RejectionCode::MissingEvidence);
    }

    if ctx.fingerprint_exists {
        rejects.push(RejectionCode::DuplicateCandidate);
    }

    // Non-place entity must never become place_entity_id.
    if candidate.place_entity_id.is_some() {
        match ctx.place_entity_kind {
            Some(EntityKind::Place) => {}
            Some(_) | None => rejects.push(RejectionCode::InvalidPlaceKind),
        }
    }

    // Also: any place_mention resolved to non-place kind is invalid if used as place_label source.
    for m in &candidate.place_mentions {
        if let Some(kind) = m.kind {
            if kind != EntityKind::Place {
                rejects.push(RejectionCode::InvalidPlaceKind);
                break;
            }
        }
    }

    let event_year = candidate.time.year_for_gates();

    if let (Some(ey), Some(by)) = (event_year, ctx.subject_birth_year) {
        if ey < by && candidate.event_type != "birth" {
            rejects.push(RejectionCode::EventBeforeSubjectBirth);
        }
        let age = ey - by;
        if candidate.event_type != "birth" && implausible_age(&candidate.event_type, age) {
            rejects.push(RejectionCode::ImplausibleAgeForEventType);
        }
    }

    if let (Some(ey), Some(dy)) = (event_year, ctx.subject_death_year) {
        if ey > dy && candidate.event_type != "death" {
            rejects.push(RejectionCode::EventAfterSubjectDeath);
        }
        // Death after known death year is also after-death for a second death attempt.
        if candidate.event_type == "death" && ey != dy && ctx.has_active_death {
            // handled by singleton; still flag lifespan if wildly after
            if ey > dy {
                rejects.push(RejectionCode::EventAfterSubjectDeath);
            }
        }
    }

    if candidate.event_type == "birth" && ctx.has_active_birth {
        rejects.push(RejectionCode::SingletonCardinalityViolation);
    }
    if candidate.event_type == "death" && ctx.has_active_death {
        rejects.push(RejectionCode::SingletonCardinalityViolation);
    }

    // Death year before birth year when both known on the candidate itself via context.
    if candidate.event_type == "death" {
        if let (Some(ey), Some(by)) = (event_year, ctx.subject_birth_year) {
            if ey < by {
                rejects.push(RejectionCode::EventBeforeSubjectBirth);
            }
        }
    }

    // Deduplicate codes while preserving order.
    let mut seen = std::collections::HashSet::new();
    rejects.retain(|c| seen.insert(*c));

    if !rejects.is_empty() {
        return GateDecision::Reject(rejects);
    }

    // Soft: unknown time → needs_review rather than hard accept for map.
    if matches!(candidate.time, TypedTime::Unknown { .. }) {
        return GateDecision::NeedsReview;
    }

    if candidate.subject_entity_id.is_none() {
        return GateDecision::NeedsReview;
    }

    GateDecision::Accept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EvidencePtr, EXTRACTOR_DETERMINISTIC_V1};
    use uuid::Uuid;

    fn base_candidate(event_type: &str, year: i32) -> EventCandidate {
        let frag = Uuid::nil();
        EventCandidate {
            id: Uuid::new_v4(),
            snapshot_id: Uuid::nil(),
            fragment_id: frag,
            clause_index: 0,
            subject_surface: "Subject".into(),
            subject_entity_id: Some(Uuid::new_v4()),
            event_type: event_type.into(),
            predicate: "occurs".into(),
            time: TypedTime::Exact {
                year,
                month: None,
                day: None,
                surface: Some(year.to_string()),
            },
            place_mentions: vec![],
            object_mentions: vec![],
            participant_mentions: vec![],
            place_entity_id: None,
            place_label: Some("Paris".into()),
            evidence_ptrs: vec![EvidencePtr {
                fragment_id: frag,
                clause_index: 0,
                start_offset: 0,
                end_offset: 10,
                quoted_text: "evidence".into(),
            }],
            extractor_version: EXTRACTOR_DETERMINISTIC_V1.into(),
            fingerprint: "fp".into(),
            status: CandidateStatus::Pending,
            rejection_codes: vec![],
        }
    }

    #[test]
    fn rejects_leipzig_style_before_birth() {
        let c = base_candidate("battle", 1774);
        let ctx = GateContext {
            subject_birth_year: Some(1769),
            ..Default::default()
        };
        // age 5 for battle → implausible; also not before birth (1774>1769)
        let d = apply_gates(&c, &ctx);
        assert!(matches!(d, GateDecision::Reject(_)));
        let codes = d.codes();
        assert!(codes.contains(&"implausible_age_for_event_type".into()));
    }

    #[test]
    fn rejects_event_before_birth() {
        let c = base_candidate("battle", 1760);
        let ctx = GateContext {
            subject_birth_year: Some(1769),
            ..Default::default()
        };
        let codes = apply_gates(&c, &ctx).codes();
        assert!(codes.contains(&"event_before_subject_birth".into()));
    }

    #[test]
    fn rejects_death_before_known_plausible_but_after_wrong_year_via_singleton() {
        let c = base_candidate("death", 1798);
        let ctx = GateContext {
            subject_birth_year: Some(1769),
            subject_death_year: Some(1821),
            has_active_death: true,
            ..Default::default()
        };
        let codes = apply_gates(&c, &ctx).codes();
        assert!(codes.contains(&"singleton_cardinality_violation".into()));
    }

    #[test]
    fn rejects_invalid_place_kind() {
        let mut c = base_candidate("marriage", 1796);
        c.place_entity_id = Some(Uuid::new_v4());
        let ctx = GateContext {
            subject_birth_year: Some(1769),
            place_entity_kind: Some(EntityKind::Person),
            ..Default::default()
        };
        let codes = apply_gates(&c, &ctx).codes();
        assert!(codes.contains(&"invalid_place_kind".into()));
    }

    #[test]
    fn rejects_cross_clause_join() {
        let c = base_candidate("battle", 1813);
        let ctx = GateContext {
            subject_birth_year: Some(1769),
            cross_clause_join_detected: true,
            ..Default::default()
        };
        let codes = apply_gates(&c, &ctx).codes();
        assert!(codes.contains(&"cross_clause_join".into()));
    }

    #[test]
    fn rejects_missing_evidence() {
        let mut c = base_candidate("battle", 1813);
        c.evidence_ptrs.clear();
        let ctx = GateContext {
            subject_birth_year: Some(1769),
            ..Default::default()
        };
        let codes = apply_gates(&c, &ctx).codes();
        assert!(codes.contains(&"missing_evidence".into()));
    }
}
