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
    CompetingPlace,
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
            Self::CompetingPlace => "competing_place",
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
            "competing_place" => Self::CompetingPlace,
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

fn looks_like_accession(evidence: &str) -> bool {
    let e = evidence.to_lowercase();
    [
        "crowned",
        "couronné",
        "couronne",
        "became king",
        "became queen",
        "became emperor",
        "became empress",
        "accession",
        "succeeded",
        "sacré roi",
        "proclaimed emperor",
        "proclamé empereur",
        "devint roi",
        "devient roi",
        "devint empereur",
        "king of",
        "queen of",
        "empereur",
    ]
    .iter()
    .any(|cue| e.contains(cue))
}

/// Generic age plausibility by event type (not subject-specific).
fn implausible_age(event_type: &str, age: i32, evidence: &str) -> bool {
    match event_type {
        "birth" => false,
        "death" => age < 0 || age > 120,
        "marriage" | "divorce" => age < 12 || age > 100,
        "battle" | "employment" | "exile" | "imprisonment" => age < 10 || age > 100,
        // Child monarchs hold office from accession; otherwise require age 12+.
        "office" | "diplomatic" => {
            if age < 0 || age > 100 {
                true
            } else if age < 12 {
                !looks_like_accession(evidence)
            } else {
                false
            }
        }
        "education" => age < 3 || age > 90,
        _ => age < 0 || age > 120,
    }
}

/// Whether an event type implies the subject was physically present (participatory).
/// About-subject types (publications, commemorations, awards) do not.
pub fn event_implies_subject_presence(event_type: &str, predicate: &str) -> bool {
    match event_type {
        "publication" | "commemoration" | "award" => false,
        _ => match predicate {
            "commemorated_at" | "published" | "awarded" => false,
            _ => true,
        },
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
        let evidence = candidate
            .evidence_ptrs
            .first()
            .map(|e| e.quoted_text.as_str())
            .unwrap_or("");
        if candidate.event_type != "birth" && implausible_age(&candidate.event_type, age, evidence) {
            rejects.push(RejectionCode::ImplausibleAgeForEventType);
        }
    }

    if let (Some(ey), Some(dy)) = (event_year, ctx.subject_death_year) {
        if ey > dy
            && candidate.event_type != "death"
            && event_implies_subject_presence(&candidate.event_type, &candidate.predicate)
        {
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

/// Publications stay on the timeline; map pins are life-locus types only.
pub fn event_type_is_map_locus(event_type: &str) -> bool {
    matches!(
        event_type,
        "birth"
            | "death"
            | "residence"
            | "arrival"
            | "departure"
            | "passage"
            | "meeting"
            | "exile"
            | "battle"
            | "siege"
            | "education"
            | "office"
            | "marriage"
            | "divorce"
            | "travel"
            | "imprisonment"
            | "diplomatic"
            | "employment"
    )
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
    fn child_monarch_office_without_accession_is_implausible() {
        let c = base_candidate("office", 1643);
        let ctx = GateContext {
            subject_birth_year: Some(1638),
            subject_death_year: Some(1715),
            ..Default::default()
        };
        let codes = apply_gates(&c, &ctx).codes();
        assert!(codes.contains(&"implausible_age_for_event_type".into()));
    }

    #[test]
    fn child_monarch_office_with_coronation_is_plausible() {
        let mut c = base_candidate("office", 1643);
        c.evidence_ptrs[0].quoted_text = "Louis XIV was crowned King of France".into();
        let ctx = GateContext {
            subject_birth_year: Some(1638),
            subject_death_year: Some(1715),
            ..Default::default()
        };
        assert!(matches!(apply_gates(&c, &ctx), GateDecision::Accept));
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

    #[test]
    fn publication_is_not_a_map_locus() {
        assert!(!event_type_is_map_locus("publication"));
        assert!(event_type_is_map_locus("arrival"));
        assert!(event_type_is_map_locus("residence"));
        assert!(!event_type_is_map_locus("historical_fact"));
    }

    #[test]
    fn participatory_anecdote_after_death_is_rejected() {
        let c = base_candidate("anecdote", 1981);
        let ctx = GateContext {
            subject_birth_year: Some(1754),
            subject_death_year: Some(1793),
            ..Default::default()
        };
        let codes = apply_gates(&c, &ctx).codes();
        assert!(codes.contains(&"event_after_subject_death".into()));
    }

    #[test]
    fn commemoration_after_death_is_not_lifespan_rejected() {
        let c = base_candidate("commemoration", 1840);
        let ctx = GateContext {
            subject_birth_year: Some(1754),
            subject_death_year: Some(1793),
            ..Default::default()
        };
        assert!(!apply_gates(&c, &ctx)
            .codes()
            .contains(&"event_after_subject_death".into()));
    }

    #[test]
    fn arrival_before_birth_is_still_rejected() {
        let c = base_candidate("arrival", 1450);
        let ctx = GateContext {
            subject_birth_year: Some(1451),
            subject_death_year: Some(1506),
            ..Default::default()
        };
        assert!(apply_gates(&c, &ctx)
            .codes()
            .contains(&"event_before_subject_birth".into()));
    }
}
