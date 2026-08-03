// crates/talaria-sources/tests/lot_a_foundations.rs
use talaria_sources::connectors::FixtureConnector;
use talaria_sources::extractors::{
    claim_fingerprint, default_extractor_stack, CandidateExtractor, ClaimKey, ExtractorInput,
};
use talaria_sources::{
    plan_sources, BudgetCounters, DiscoveryCursor, IngestBudgets, ResolvedSubject, SourceConnector,
    SourceKind,
};

#[tokio::test]
async fn discovery_paginated_with_resume() {
    let fx = FixtureConnector::dense_biography_pack("Subject");
    let subject = ResolvedSubject {
        entity_id: None,
        qid: None,
        label: "Subject".into(),
        languages: vec!["en".into()],
        birth_year: Some(1769),
        death_year: Some(1821),
        countries: vec![],
        occupations: vec![],
        known_identifiers: vec![],
    };
    let page1 = fx.discover(&subject, None).await.unwrap();
    assert_eq!(page1.documents.len(), 2);
    assert!(page1.next_cursor.is_some());
    let page2 = fx
        .discover(&subject, page1.next_cursor)
        .await
        .unwrap();
    assert!(!page2.documents.is_empty());
    // Resume further
    let mut cursor = page2.next_cursor;
    let mut total = page1.documents.len() + page2.documents.len();
    while let Some(c) = cursor {
        let page = fx.discover(&subject, Some(c)).await.unwrap();
        total += page.documents.len();
        cursor = page.next_cursor;
    }
    assert!(total >= 4);
}

#[tokio::test]
async fn snapshot_content_stable_across_fetch() {
    let fx = FixtureConnector::dense_biography_pack("Subject");
    let subject = ResolvedSubject {
        entity_id: None,
        qid: None,
        label: "Subject".into(),
        languages: vec!["en".into()],
        birth_year: None,
        death_year: None,
        countries: vec![],
        occupations: vec![],
        known_identifiers: vec![],
    };
    let page = fx.discover(&subject, None).await.unwrap();
    let a = fx.fetch(&page.documents[0]).await.unwrap();
    let b = fx.fetch(&page.documents[0]).await.unwrap();
    assert_eq!(a.text, b.text);
    assert_eq!(a.revision_id, b.revision_id);
}

#[test]
fn claim_fingerprint_merges_same_fact() {
    let k = ClaimKey {
        subject: "Napoleon".into(),
        predicate: "died_in".into(),
        object_or_value: "".into(),
        time_key: "exact:1821:0:0".into(),
        place_key: "Saint Helena".into(),
    };
    assert_eq!(claim_fingerprint(&k), claim_fingerprint(&k));
}

#[test]
fn josephine_not_place_via_extractors_object_path() {
    // Structured statement with person place should be handled at resolve layer;
    // extractor keeps surface separate.
    let ex = default_extractor_stack();
    let input = ExtractorInput {
        text: "STATEMENT\tmarriage\tmarried\t1796\tParis\n".into(),
        page_title: Some("Subject".into()),
        document_type: "structured_statement".into(),
        subject_death_year: Some(1821),
    };
    let mut found = false;
    for e in &ex {
        for c in e.extract(&input) {
            if c.event_type == "marriage" {
                assert_eq!(c.place_surface.as_deref(), Some("Paris"));
                found = true;
            }
        }
    }
    assert!(found);
}

#[test]
fn low_relevance_doc_produces_no_forced_candidates_when_skipped() {
    // Document itself may extract nothing useful from newsletter text.
    let ex = default_extractor_stack();
    let input = ExtractorInput {
        text: "Market prices rose in Lyon. Weather was mild.".into(),
        page_title: Some("Unrelated".into()),
        document_type: "press_ocr".into(),
        subject_death_year: None,
    };
    let mut n = 0;
    for e in &ex {
        n += e.extract(&input).len();
    }
    assert_eq!(n, 0);
}

#[test]
fn budget_limits_respected() {
    let budgets = IngestBudgets {
        max_documents_per_source: 2,
        max_external_calls: 10,
        ..IngestBudgets::default()
    };
    let mut c = BudgetCounters::default();
    assert!(c.record_document("fixture", &budgets, 10).is_ok());
    assert!(c.record_document("fixture", &budgets, 10).is_ok());
    assert!(c.record_document("fixture", &budgets, 10).is_err());
}

#[test]
fn plan_sources_for_french_military() {
    let subject = ResolvedSubject {
        entity_id: None,
        qid: Some("Q517".into()),
        label: "Someone".into(),
        languages: vec!["fr".into()],
        birth_year: Some(1769),
        death_year: Some(1821),
        countries: vec!["France".into()],
        occupations: vec!["military".into()],
        known_identifiers: vec![],
    };
    let plan = plan_sources(&subject, IngestBudgets::default());
    assert!(plan.sources.iter().any(|s| s.kind == SourceKind::Wikidata));
    assert!(plan.sources.iter().any(|s| s.kind == SourceKind::Bnf));
    assert!(plan.sources.iter().any(|s| s.kind == SourceKind::Gallica));
}

#[test]
fn posthumous_typed_as_commemoration() {
    let ex = default_extractor_stack();
    let input = ExtractorInput {
        text: "* 1840 — Paris — remains returned\n".into(),
        page_title: Some("Subject".into()),
        document_type: "chronology_list".into(),
        subject_death_year: Some(1821),
    };
    let mut found = false;
    for e in &ex {
        for c in e.extract(&input) {
            if c.time_surface.as_deref() == Some("1840") {
                assert_eq!(c.event_type, "commemoration");
                assert!(c.is_posthumous);
                found = true;
            }
        }
    }
    assert!(found);
}

#[tokio::test]
async fn stub_connector_does_not_invent_results() {
    use talaria_sources::connectors::StubConnector;
    let stub = StubConnector::new(SourceKind::Europeana, "needs EUROPEANA_API_KEY");
    let subject = ResolvedSubject {
        entity_id: None,
        qid: None,
        label: "X".into(),
        languages: vec![],
        birth_year: None,
        death_year: None,
        countries: vec![],
        occupations: vec![],
        known_identifiers: vec![],
    };
    assert!(stub.discover(&subject, None).await.is_err());
}

#[tokio::test]
async fn cursor_offset_zero_default() {
    let c = DiscoveryCursor::default();
    assert_eq!(c.offset, 0);
}
