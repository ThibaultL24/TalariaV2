// crates/talaria-api/src/person_ingest/gating.rs
//! Build EventCandidate, apply_gates, and classify_attribution.

use talaria_quality::{
    apply_gates, attribution_gate_decision, candidate_fingerprint, classify_attribution,
    AttributionInput, AttributionMatch, CandidateStatus, EventCandidate, EvidencePtr, GateContext,
    GateDecision, GroundedItem, TypedTime, EXTRACTOR_DETERMINISTIC_V1,
};
use uuid::Uuid;

pub fn fingerprint_for(
    subject: &str,
    item: &GroundedItem,
    time: &TypedTime,
    raw_document_id: Uuid,
) -> String {
    candidate_fingerprint(
        EXTRACTOR_DETERMINISTIC_V1,
        subject,
        &item.event_type,
        &item.role,
        time,
        item.place_surface.as_deref(),
        &raw_document_id.to_string(),
        0,
        0,
        item.quoted_text.len() as i32,
        &[],
    )
}

pub fn event_candidate_from_item(
    entity_id: Uuid,
    subject: &str,
    item: &GroundedItem,
    time: &TypedTime,
    fingerprint: &str,
) -> EventCandidate {
    let frag = Uuid::nil();
    EventCandidate {
        id: Uuid::nil(),
        snapshot_id: Uuid::nil(),
        fragment_id: frag,
        clause_index: 0,
        subject_surface: subject.to_string(),
        subject_entity_id: Some(entity_id),
        event_type: item.event_type.clone(),
        predicate: item.role.clone(),
        time: time.clone(),
        place_mentions: vec![],
        object_mentions: vec![],
        participant_mentions: vec![],
        place_entity_id: None,
        place_label: item.place_surface.clone(),
        evidence_ptrs: vec![EvidencePtr {
            fragment_id: frag,
            clause_index: 0,
            start_offset: 0,
            end_offset: item.quoted_text.len() as i32,
            quoted_text: item.quoted_text.clone(),
        }],
        extractor_version: EXTRACTOR_DETERMINISTIC_V1.into(),
        fingerprint: fingerprint.to_string(),
        status: CandidateStatus::Pending,
        rejection_codes: vec![],
    }
}

pub fn classify_item(
    subject: &str,
    aliases: &[String],
    item: &GroundedItem,
    page_title: &str,
    from_followed_page: bool,
    structured_source: bool,
) -> AttributionMatch {
    let alias_refs: Vec<&str> = aliases.iter().map(String::as_str).collect();
    let role_supported_by_evidence = structured_source
        || alias_refs
            .iter()
            .any(|a| item.quoted_text.to_lowercase().contains(&a.to_lowercase()))
        || item
            .quoted_text
            .to_lowercase()
            .contains(&subject.to_lowercase());
    classify_attribution(&AttributionInput {
        subject,
        aliases: &alias_refs,
        quote: &item.quoted_text,
        page_title,
        from_followed_page,
        structured_source,
        role_supported_by_evidence,
    })
}

pub fn merge_gate_and_attribution(
    decision: GateDecision,
    attribution: AttributionMatch,
) -> GateDecision {
    let attr = attribution_gate_decision(attribution);
    match (decision, attr) {
        (GateDecision::Reject(mut a), GateDecision::Reject(b)) => {
            a.extend(b);
            GateDecision::Reject(a)
        }
        (GateDecision::Reject(a), _) => GateDecision::Reject(a),
        (_, GateDecision::Reject(b)) => GateDecision::Reject(b),
        (GateDecision::NeedsReview, _) | (_, GateDecision::NeedsReview) => GateDecision::NeedsReview,
        (GateDecision::Accept, GateDecision::Accept) => GateDecision::Accept,
    }
}

pub fn judge_item(
    candidate: &EventCandidate,
    ctx: &GateContext,
    attribution: AttributionMatch,
) -> GateDecision {
    merge_gate_and_attribution(apply_gates(candidate, ctx), attribution)
}

#[cfg(test)]
mod tests {
    use super::*;
    use talaria_quality::{RejectionCode, TypedTime};

    fn anecdote_candidate(year: i32) -> EventCandidate {
        let frag = Uuid::nil();
        EventCandidate {
            id: Uuid::nil(),
            snapshot_id: Uuid::nil(),
            fragment_id: frag,
            clause_index: 0,
            subject_surface: "Louis XVI".into(),
            subject_entity_id: Some(Uuid::nil()),
            event_type: "anecdote".into(),
            predicate: "direct".into(),
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
            place_label: None,
            evidence_ptrs: vec![EvidencePtr {
                fragment_id: frag,
                clause_index: 0,
                start_offset: 0,
                end_offset: 8,
                quoted_text: "anecdote".into(),
            }],
            extractor_version: EXTRACTOR_DETERMINISTIC_V1.into(),
            fingerprint: "fp".into(),
            status: CandidateStatus::Pending,
            rejection_codes: vec![],
        }
    }

    #[test]
    fn louis_xvi_1981_anecdote_rejects() {
        let c = anecdote_candidate(1981);
        let ctx = GateContext {
            subject_death_year: Some(1793),
            ..Default::default()
        };
        match apply_gates(&c, &ctx) {
            GateDecision::Reject(codes) => {
                assert!(codes.contains(&RejectionCode::EventAfterSubjectDeath));
            }
            other => panic!("expected Reject, got {other:?}"),
        }
    }
}
