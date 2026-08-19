// crates/talaria-quality/tests/napoleon_gates.rs
//! Deterministic Napoleon fixture tests — no biography-specific hardcoded rules.
//! Gates are generic; fixture data exercises them.

use talaria_quality::{
    apply_gates, candidate_fingerprint, event_fingerprint, resolve_mentions, split_clauses,
    CandidateStatus, ClauseAnalyzeInput, ClauseAnalyzer, DeterministicClauseAnalyzer, EntityKind,
    EvidencePtr, GateContext, GateDecision, GazetteerResolver, Mention, ParticipantRole, TypedTime,
    EXTRACTOR_DETERMINISTIC_V1,
};
use uuid::Uuid;

fn cand(
    event_type: &str,
    year: i32,
    place_label: Option<&str>,
    place_entity: Option<(Uuid, EntityKind)>,
    evidence: bool,
) -> talaria_quality::EventCandidate {
    let frag = Uuid::new_v4();
    let mut c = talaria_quality::EventCandidate {
        id: Uuid::new_v4(),
        snapshot_id: Uuid::new_v4(),
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
        place_entity_id: place_entity.map(|(id, _)| id),
        place_label: place_label.map(str::to_string),
        evidence_ptrs: if evidence {
            vec![EvidencePtr {
                fragment_id: frag,
                clause_index: 0,
                start_offset: 0,
                end_offset: 12,
                quoted_text: "quoted".into(),
            }]
        } else {
            vec![]
        },
        extractor_version: EXTRACTOR_DETERMINISTIC_V1.into(),
        fingerprint: "fp".into(),
        status: CandidateStatus::Pending,
        rejection_codes: vec![],
    };
    if let Some((id, kind)) = place_entity {
        c.place_mentions.push(Mention {
            surface: place_label.unwrap_or("").into(),
            entity_id: Some(id),
            kind: Some(kind),
            role: None,
        });
    }
    c
}

#[test]
fn leipzig_1774_rejected_implausible_age() {
    let c = cand("battle", 1774, Some("Leipzig"), None, true);
    let ctx = GateContext {
        subject_birth_year: Some(1769),
        ..Default::default()
    };
    let codes = apply_gates(&c, &ctx).codes();
    assert!(
        codes.contains(&"implausible_age_for_event_type".into()),
        "got {codes:?}"
    );
}

#[test]
fn waterloo_1798_death_rejected_singleton() {
    let c = cand("death", 1798, Some("Waterloo"), None, true);
    let ctx = GateContext {
        subject_birth_year: Some(1769),
        subject_death_year: Some(1821),
        has_active_death: true,
        ..Default::default()
    };
    let codes = apply_gates(&c, &ctx).codes();
    assert!(
        codes.contains(&"singleton_cardinality_violation".into()),
        "got {codes:?}"
    );
}

#[test]
fn josephine_never_resolved_as_place() {
    let mut c = cand("marriage", 1796, None, None, true);
    c.subject_surface = "Napoleon".into();
    let r = resolve_mentions(&c, &GazetteerResolver, Some("Joséphine"), None, &[]);
    assert!(r.invalid_place_attempt);
    assert!(r.place_label.is_none());
    assert_eq!(r.participant_mentions[0].kind, Some(EntityKind::Person));
    assert_eq!(
        r.participant_mentions[0].role,
        Some(ParticipantRole::Spouse)
    );
    // Even if legacy code forced place_entity_id to a person, gates reject.
    let person_id = Uuid::new_v4();
    c.place_entity_id = Some(person_id);
    c.place_label = Some("Joséphine".into());
    c.place_mentions = vec![Mention {
        surface: "Joséphine".into(),
        entity_id: Some(person_id),
        kind: Some(EntityKind::Person),
        role: None,
    }];
    let ctx = GateContext {
        subject_birth_year: Some(1769),
        place_entity_kind: Some(EntityKind::Person),
        ..Default::default()
    };
    let codes = apply_gates(&c, &ctx).codes();
    assert!(codes.contains(&"invalid_place_kind".into()));
}

#[test]
fn paris_resolves_as_place() {
    let c = cand("marriage", 1796, None, None, true);
    let r = resolve_mentions(&c, &GazetteerResolver, Some("Paris"), None, &[]);
    assert_eq!(r.place_kind, Some(EntityKind::Place));
}

#[test]
fn no_cross_clause_join_from_analyzer() {
    let analyzer = DeterministicClauseAnalyzer;
    let xs = analyzer.analyze_sentence(&ClauseAnalyzeInput {
        text: "In 1774 his father died; he fought in Leipzig.".into(),
        page_title: Some("Napoleon".into()),
        start_offset: 0,
    });
    assert!(xs.iter().all(|x| !x.cross_clause_join));
    for x in xs.iter().filter(|x| x.event_type == "battle") {
        assert_ne!(x.time_surface.as_deref(), Some("1774"));
    }
}

#[test]
fn cross_clause_gate_rejects() {
    let c = cand("battle", 1813, Some("Leipzig"), None, true);
    let ctx = GateContext {
        subject_birth_year: Some(1769),
        cross_clause_join_detected: true,
        ..Default::default()
    };
    assert!(apply_gates(&c, &ctx)
        .codes()
        .contains(&"cross_clause_join".into()));
}

#[test]
fn retry_fingerprint_stable() {
    let t = TypedTime::Exact {
        year: 1821,
        month: None,
        day: None,
        surface: Some("1821".into()),
    };
    let a = candidate_fingerprint(
        EXTRACTOR_DETERMINISTIC_V1,
        "Napoleon",
        "death",
        "died_in",
        &t,
        Some("Saint Helena"),
        "snap",
        0,
        0,
        40,
        &[],
    );
    let b = candidate_fingerprint(
        EXTRACTOR_DETERMINISTIC_V1,
        "Napoleon",
        "death",
        "died_in",
        &t,
        Some("Saint Helena"),
        "snap",
        0,
        0,
        40,
        &[],
    );
    assert_eq!(a, b);
    let ea = event_fingerprint("subj", "death", "died_in", &t, Some("place"), &[]);
    let eb = event_fingerprint("subj", "death", "died_in", &t, Some("place"), &[]);
    assert_eq!(ea, eb);
}

#[test]
fn missing_evidence_rejected() {
    let c = cand("battle", 1813, Some("Leipzig"), None, false);
    let ctx = GateContext {
        subject_birth_year: Some(1769),
        ..Default::default()
    };
    assert!(matches!(apply_gates(&c, &ctx), GateDecision::Reject(_)));
}

#[test]
fn clause_split_separates_signals() {
    let clauses = split_clauses("In 1774 his father died; he fought in Leipzig.");
    assert!(clauses.len() >= 2);
}
