// crates/talaria-quality/tests/person_pipeline_regressions.rs
//! Measured DB regression cases — pure gate/attribution/time, no network.

use talaria_quality::{
    apply_gates, attribution_gate_decision, classify_attribution, place_query,
    AttributionInput, AttributionMatch, CandidateStatus, EventCandidate, EvidencePtr,
    GateContext, GateDecision, TypedTime, EXTRACTOR_DETERMINISTIC_V1,
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
