// crates/talaria-sources/tests/openalex_corpus.rs
use std::path::PathBuf;

use talaria_sources::connectors::OpenAlexConnector;
use talaria_sources::{
    match_subject_to_document, normalize_openalex_work, scan_bibliographic, AcademicStatus,
    AccessLevel, DebateType, DiscoveryCursor, DocumentType, IdentifierScheme, ResolvedSubject,
    SourceConnector, SourceKind,
};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/open_alex")
}

fn subject_columbus() -> ResolvedSubject {
    ResolvedSubject {
        entity_id: None,
        qid: Some("Q7322".into()),
        label: "Christopher Columbus".into(),
        languages: vec!["en".into(), "fr".into()],
        birth_year: Some(1451),
        death_year: Some(1506),
        countries: vec![],
        occupations: vec![],
        known_identifiers: vec![],
    }
}

#[test]
fn reconstructs_abstract_and_marks_origin_paper_peer_reviewed() {
    let raw = std::fs::read_to_string(fixture_dir().join("details/W4210000001.json")).unwrap();
    let detail: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let n = normalize_openalex_work(&detail).unwrap();
    assert_eq!(n.source_kind, SourceKind::OpenAlex);
    assert_eq!(n.external_id, "W4210000001");
    assert_eq!(n.document_type, DocumentType::AcademicArticle);
    assert_eq!(n.academic_status, AcademicStatus::PeerReviewed);
    assert_eq!(n.access_level, AccessLevel::Open);
    assert!(n
        .identifiers
        .iter()
        .any(|i| i.scheme == IdentifierScheme::Doi
            && i.value_normalized == "10.0000/columbus.origins"));
    let abs = n.abstract_text.as_deref().unwrap();
    assert!(abs.contains("origins"));
    assert!(abs.contains("Columbus"));
    assert!(abs.contains("Genoese"));
}

#[test]
fn closed_work_is_metadata_only() {
    let raw = std::fs::read_to_string(fixture_dir().join("details/W4210000002.json")).unwrap();
    let detail: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let n = normalize_openalex_work(&detail).unwrap();
    assert_eq!(n.access_level, AccessLevel::MetadataOnly);
    assert!(!n.full_text_available);
}

#[test]
fn columbus_origin_title_is_a_historiography_hit() {
    let hits = scan_bibliographic("Origin theories of Christopher Columbus", None);
    assert_eq!(hits[0].debate_type, DebateType::IdentityOriginDispute);
}

#[test]
fn polymer_title_is_not_a_debate() {
    let hits = scan_bibliographic("Advances in polymer chemistry", None);
    assert!(hits.is_empty());
}

#[tokio::test]
async fn openalex_discover_paginates_fixtures() {
    let conn = OpenAlexConnector::from_fixture_dir(fixture_dir()).unwrap();
    let page1 = conn.discover(&subject_columbus(), None).await.unwrap();
    assert_eq!(page1.documents.len(), 2);
    assert!(page1.next_cursor.is_none());

    let cfg = talaria_sources::connectors::OpenAlexConfig {
        fixture_dir: Some(fixture_dir()),
        page_size: 1,
        ..talaria_sources::connectors::OpenAlexConfig::default()
    };
    let small = OpenAlexConnector::new(cfg).unwrap();
    let p1 = small.discover(&subject_columbus(), None).await.unwrap();
    assert_eq!(p1.documents.len(), 1);
    assert!(p1.next_cursor.is_some());
    let p2 = small
        .discover(&subject_columbus(), p1.next_cursor)
        .await
        .unwrap();
    assert_eq!(p2.documents.len(), 1);
    assert!(p2.next_cursor.is_none());
}

#[tokio::test]
async fn openalex_fetch_links_columbus_paper_to_subject() {
    let conn = OpenAlexConnector::from_fixture_dir(fixture_dir()).unwrap();
    let page = conn.discover(&subject_columbus(), None).await.unwrap();
    let origin = page
        .documents
        .iter()
        .find(|d| d.external_id == "W4210000001")
        .unwrap();
    let fetched = conn.fetch(origin).await.unwrap();
    let n: talaria_sources::NormalizedCorpusDocument =
        serde_json::from_value(fetched.raw_metadata.get("normalized").cloned().unwrap()).unwrap();
    let m = match_subject_to_document("Christopher Columbus", &n).unwrap();
    assert!(m.score > 0.0);
}

#[tokio::test]
async fn registry_lists_openalex_when_wired() {
    let conn = OpenAlexConnector::from_fixture_dir(fixture_dir()).unwrap();
    let reg = talaria_sources::connectors::default_registry_corpus(None, false, None, Some(conn))
        .unwrap();
    let oa = reg.get(&SourceKind::OpenAlex).unwrap();
    assert!(oa.implemented);
}

#[test]
fn debate_query_includes_origin_terms() {
    let q = talaria_sources::connectors::openalex_debate_query("Christophe Colomb");
    assert!(q.contains("Christophe Colomb"));
    assert!(q.contains("origins") || q.contains("origines"));
}

#[test]
fn cursor_offset_zero_is_page_one() {
    let c = DiscoveryCursor::default();
    assert_eq!(c.offset, 0);
}
