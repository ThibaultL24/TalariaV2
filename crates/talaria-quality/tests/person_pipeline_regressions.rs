// crates/talaria-quality/tests/person_pipeline_regressions.rs
//! Measured DB regression cases — pure gate/attribution/time, no network.

use talaria_quality::{
    apply_gates, approx_typed_time, attribution_gate_decision, classify_attribution,
    infer_approximate_year, is_approx_eligible_type, place_query, AttributionInput,
    AttributionMatch, CandidateStatus, EventCandidate, EvidencePtr, GateContext, GateDecision,
    TypedTime, EXTRACTOR_DETERMINISTIC_V1,
};
use uuid::Uuid;

fn subject_candidate(
    subject: &str,
    event_type: &str,
    year: i32,
    quote: &str,
) -> EventCandidate {
    let frag = Uuid::new_v4();
    EventCandidate {
        id: Uuid::new_v4(),
        snapshot_id: Uuid::new_v4(),
        fragment_id: frag,
        clause_index: 0,
        subject_surface: subject.into(),
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
        place_label: None,
        evidence_ptrs: vec![EvidencePtr {
            fragment_id: frag,
            clause_index: 0,
            start_offset: 0,
            end_offset: quote.len() as i32,
            quoted_text: quote.into(),
        }],
        extractor_version: EXTRACTOR_DETERMINISTIC_V1.into(),
        fingerprint: "fp".into(),
        status: CandidateStatus::Pending,
        rejection_codes: vec![],
    }
}

fn assert_rejects_with(codes: &[String], expected: &str) {
    assert!(
        codes.contains(&expected.to_string()),
        "expected {expected} in {codes:?}"
    );
}

/// DB: `Louis XVI — anecdote (1981)` must not pass lifespan gates.
#[test]
fn louis_xvi_anecdote_1981_rejected_after_death() {
    let c = subject_candidate(
        "Louis XVI",
        "anecdote",
        1981,
        "An anecdote about Louis XVI in 1981.",
    );
    let ctx = GateContext {
        subject_birth_year: Some(1754),
        subject_death_year: Some(1793),
        ..Default::default()
    };
    let decision = apply_gates(&c, &ctx);
    assert!(matches!(decision, GateDecision::Reject(_)));
    assert_rejects_with(&decision.codes(), "event_after_subject_death");
}

/// DB: `Victor Hugo — battle (1884)` from a followed battle page without role evidence.
#[test]
fn victor_hugo_followed_battle_page_is_unattributed() {
    let m = classify_attribution(&AttributionInput {
        subject: "Victor Hugo",
        aliases: &["Hugo"],
        quote: "The Battle of Plevna was fought in 1877.",
        page_title: "Siege of Plevna",
        event_type: "battle",
        from_followed_page: true,
        structured_source: false,
        role_supported_by_evidence: false,
        military_subject: false,
    });
    assert_eq!(m, AttributionMatch::Unattributed);
    let gate = attribution_gate_decision(m);
    assert!(matches!(gate, GateDecision::Reject(_)));
    assert_rejects_with(&gate.codes(), "subject_not_attributed");
}

/// DB: `Christopher Columbus — arrival (1453)` — use 1450 (before birth 1451).
#[test]
fn columbus_arrival_before_birth_is_rejected() {
    let c = subject_candidate(
        "Christopher Columbus",
        "arrival",
        1450,
        "Columbus arrived in the Americas.",
    );
    let ctx = GateContext {
        subject_birth_year: Some(1451),
        subject_death_year: Some(1506),
        ..Default::default()
    };
    let decision = apply_gates(&c, &ctx);
    assert!(matches!(decision, GateDecision::Reject(_)));
    assert_rejects_with(&decision.codes(), "event_before_subject_birth");
}

/// DB: place resolution must not strip leading articles from `place_surface`.
#[test]
fn the_hague_surface_preserved() {
    let q = place_query("The Hague");
    assert_eq!(q.surface, "The Hague");
    assert!(q.search_keys.iter().any(|k| k == "The Hague"));
    assert!(q.search_keys.iter().any(|k| k == "Hague"));
}

const VALID_TIME_KINDS: &[&str] = &["exact", "range", "approx", "unknown"];
const INVALID_TIME_KINDS: &[&str] = &["year", "month", "day"];

fn assert_time_json_kind_contract(time: &TypedTime) {
    let v = talaria_quality::time_to_json(time);
    let kind = v["kind"].as_str().expect("kind string");
    assert!(
        VALID_TIME_KINDS.contains(&kind),
        "kind must be semantic, got {kind:?} in {v}"
    );
    assert!(
        !INVALID_TIME_KINDS.contains(&kind),
        "kind must never be precision tier: {kind:?}"
    );
}

/// Task 1 contract: `time_json.kind` is semantic, never `year|month|day`.
#[test]
fn time_json_kind_never_uses_precision_as_kind() {
    let samples = [
        TypedTime::Exact {
            year: 1805,
            month: None,
            day: None,
            surface: Some("1805".into()),
        },
        TypedTime::Exact {
            year: 1805,
            month: Some(3),
            day: None,
            surface: Some("March 1805".into()),
        },
        TypedTime::Exact {
            year: 1805,
            month: Some(3),
            day: Some(15),
            surface: Some("15 March 1805".into()),
        },
        TypedTime::Range {
            start_year: 1800,
            end_year: 1810,
            surface: Some("1800–1810".into()),
        },
        TypedTime::Approx {
            year: 1450,
            surface: Some("c. 1450".into()),
        },
        TypedTime::Unknown { surface: None },
    ];
    for t in samples {
        assert_time_json_kind_contract(&t);
    }
}

// ============================================================================
// Context-aware approximate year inference tests
// ============================================================================

/// Baudelaire-like: undated trial clause with nearby year becomes canonical.
#[test]
fn baudelaire_trial_infers_year_from_paragraph_context() {
    let clause = "Baudelaire was successfully prosecuted for creating an offense against public morals.";
    let paragraph = "In 1857, Les Fleurs du mal was published. Baudelaire was successfully prosecuted for creating an offense against public morals. Six poems were condemned.";
    
    let result = infer_approximate_year(
        "trial",
        clause,
        Some(paragraph),
        None,
        Some(1821), // Baudelaire birth
        Some(1867), // Baudelaire death
    );
    
    assert!(result.is_some(), "should infer year from paragraph context");
    let (year, source) = result.unwrap();
    assert_eq!(year, 1857, "should infer 1857 from paragraph");
    assert_eq!(source, "paragraph_context");
}

/// Baudelaire-like: burial event infers death year.
#[test]
fn baudelaire_burial_infers_death_year() {
    let clause = "He was buried at the Cimetière du Montparnasse.";
    
    let result = infer_approximate_year(
        "burial",
        clause,
        None,
        None,
        Some(1821),
        Some(1867),
    );
    
    assert!(result.is_some(), "burial should infer from death year");
    let (year, source) = result.unwrap();
    assert_eq!(year, 1867, "burial year should be death year");
    assert_eq!(source, "death_year_for_burial");
}

/// Baudelaire-like: section heading with years provides context.
#[test]
fn baudelaire_event_infers_from_section_heading() {
    let clause = "He met Félicien Rops.";
    let paragraph = "Many artists visited him. He met Félicien Rops.";
    let section = "### Exil en Belgique (1864-1866)";
    
    let result = infer_approximate_year(
        "meeting",
        clause,
        Some(paragraph),
        Some(section),
        Some(1821),
        Some(1867),
    );
    
    assert!(result.is_some(), "should infer from section heading");
    let (year, source) = result.unwrap();
    assert_eq!(year, 1865, "should infer midpoint of 1864-1866");
    assert_eq!(source, "section_heading");
}

/// Baudelaire-like: conseil judiciaire (legal_event) with date in paragraph.
#[test]
fn baudelaire_conseil_judiciaire_infers_from_context() {
    let clause = "En septembre, un conseil judiciaire lui est imposé.";
    let paragraph = "En 1844, la famille observe la dilapidation de sa fortune. En septembre, un conseil judiciaire lui est imposé.";
    
    let result = infer_approximate_year(
        "legal_event",
        clause,
        Some(paragraph),
        None,
        Some(1821),
        Some(1867),
    );
    
    assert!(result.is_some(), "should infer from paragraph");
    let (year, _) = result.unwrap();
    assert_eq!(year, 1844);
}

/// Eligible event types for approximate inference.
#[test]
fn approx_eligible_types_include_anecdote_types() {
    assert!(is_approx_eligible_type("trial"));
    assert!(is_approx_eligible_type("legal_event"));
    assert!(is_approx_eligible_type("meeting"));
    assert!(is_approx_eligible_type("health_event"));
    assert!(is_approx_eligible_type("burial"));
    assert!(is_approx_eligible_type("education"));
    assert!(is_approx_eligible_type("political_event"));
    assert!(is_approx_eligible_type("financial_event"));
        assert!(is_approx_eligible_type("residence"));
        assert!(is_approx_eligible_type("travel"));
        assert!(is_approx_eligible_type("battle"));
        assert!(is_approx_eligible_type("siege"));
        assert!(is_approx_eligible_type("work"));
        assert!(is_approx_eligible_type("imprisonment"));
    
    // Non-eligible types
    assert!(!is_approx_eligible_type("publication"));
    assert!(!is_approx_eligible_type("birth"));
    assert!(!is_approx_eligible_type("death"));
}

/// Approximate typed time has correct kind.
#[test]
fn approx_typed_time_has_kind_approx() {
    let t = approx_typed_time(1857, "paragraph_context");
    match &t {
        TypedTime::Approx { year, surface } => {
            assert_eq!(*year, 1857);
            assert!(surface.is_some());
            assert!(surface.as_ref().unwrap().contains("inferred"));
        }
        _ => panic!("expected TypedTime::Approx"),
    }
    
    // Verify time_to_json produces valid kind
    assert_time_json_kind_contract(&t);
}

/// Events with approximate time should pass gates (not needs_review).
#[test]
fn approx_time_event_passes_gates() {
    let frag = Uuid::new_v4();
    let c = EventCandidate {
        id: Uuid::new_v4(),
        snapshot_id: Uuid::new_v4(),
        fragment_id: frag,
        clause_index: 0,
        subject_surface: "Baudelaire".into(),
        subject_entity_id: Some(Uuid::new_v4()),
        event_type: "trial".into(),
        predicate: "prosecuted".into(),
        time: approx_typed_time(1857, "paragraph_context"),
        place_mentions: vec![],
        object_mentions: vec![],
        participant_mentions: vec![],
        place_entity_id: None,
        place_label: Some("Paris".into()),
        evidence_ptrs: vec![EvidencePtr {
            fragment_id: frag,
            clause_index: 0,
            start_offset: 0,
            end_offset: 50,
            quoted_text: "Baudelaire was prosecuted for public offense.".into(),
        }],
        extractor_version: EXTRACTOR_DETERMINISTIC_V1.into(),
        fingerprint: "fp".into(),
        status: CandidateStatus::Pending,
        rejection_codes: vec![],
    };
    
    let ctx = GateContext {
        subject_birth_year: Some(1821),
        subject_death_year: Some(1867),
        ..Default::default()
    };
    
    let decision = apply_gates(&c, &ctx);
    assert!(
        !matches!(decision, GateDecision::NeedsReview),
        "approx time should not trigger needs_review: {decision:?}"
    );
    assert!(
        !matches!(decision, GateDecision::Reject(_)),
        "approx time in valid range should not reject: {decision:?}"
    );
}

/// Unknown time still triggers needs_review (regression guard).
#[test]
fn unknown_time_still_needs_review() {
    let frag = Uuid::new_v4();
    let c = EventCandidate {
        id: Uuid::new_v4(),
        snapshot_id: Uuid::new_v4(),
        fragment_id: frag,
        clause_index: 0,
        subject_surface: "Test".into(),
        subject_entity_id: Some(Uuid::new_v4()),
        event_type: "trial".into(),
        predicate: "prosecuted".into(),
        time: TypedTime::Unknown { surface: None },
        place_mentions: vec![],
        object_mentions: vec![],
        participant_mentions: vec![],
        place_entity_id: None,
        place_label: Some("Paris".into()),
        evidence_ptrs: vec![EvidencePtr {
            fragment_id: frag,
            clause_index: 0,
            start_offset: 0,
            end_offset: 20,
            quoted_text: "Something happened.".into(),
        }],
        extractor_version: EXTRACTOR_DETERMINISTIC_V1.into(),
        fingerprint: "fp".into(),
        status: CandidateStatus::Pending,
        rejection_codes: vec![],
    };
    
    let ctx = GateContext::default();
    let decision = apply_gates(&c, &ctx);
    assert!(
        matches!(decision, GateDecision::NeedsReview),
        "unknown time should still trigger needs_review: {decision:?}"
    );
}
