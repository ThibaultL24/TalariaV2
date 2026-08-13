// crates/talaria-sources/tests/heritage_corpus.rs
use std::path::PathBuf;

use talaria_sources::connectors::{
    BnfConnector, CorpusConnectors, EuropeanaConnector, InternetArchiveConnector,
};
use talaria_sources::{
    normalize_bnf_notice, normalize_europeana_item, normalize_ia_item, AcademicStatus,
    AccessLevel, DocumentType, ResolvedSubject, SourceConnector, SourceKind,
};

fn subject_columbus() -> ResolvedSubject {
    ResolvedSubject {
        entity_id: None,
        qid: Some("Q7322".into()),
        label: "Christophe Colomb".into(),
        languages: vec!["fr".into(), "en".into()],
        birth_year: Some(1451),
        death_year: Some(1506),
        countries: vec![],
        occupations: vec![],
        known_identifiers: vec![],
    }
}

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name)
}

#[test]
fn ia_origin_book_is_catalog_notice_not_an_event() {
    let raw = std::fs::read_to_string(dir("internet_archive").join("details/columbusorigins00harr.json"))
        .unwrap();
    let detail: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let n = normalize_ia_item(&detail).unwrap();
    assert_eq!(n.source_kind, SourceKind::InternetArchive);
    assert_eq!(n.external_id, "columbusorigins00harr");
    assert_eq!(n.document_type, DocumentType::BibliographicNotice);
    assert_eq!(n.academic_status, AcademicStatus::CatalogRecord);
    assert!(!n.full_text_available);
    assert!(n.title.to_lowercase().contains("origine"));
}

#[tokio::test]
async fn ia_discover_from_fixture() {
    let conn = InternetArchiveConnector::from_fixture_dir(dir("internet_archive")).unwrap();
    let page = conn.discover(&subject_columbus(), None).await.unwrap();
    assert_eq!(page.documents.len(), 2);
    assert!(page
        .documents
        .iter()
        .any(|d| d.external_id == "columbusorigins00harr"));
}

#[test]
fn europeana_item_keeps_provider_and_description() {
    let raw =
        std::fs::read_to_string(dir("europeana").join("details/123_columbus_origins.json")).unwrap();
    let detail: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let n = normalize_europeana_item(&detail).unwrap();
    assert_eq!(n.source_kind, SourceKind::Europeana);
    assert_eq!(n.external_id, "/123/columbus_origins");
    assert_eq!(n.access_level, AccessLevel::MetadataOnly);
    assert!(n
        .publisher_or_institution
        .as_deref()
        .unwrap()
        .contains("nationale"));
}

#[tokio::test]
async fn europeana_discover_from_fixture_without_api_key() {
    let conn = EuropeanaConnector::from_fixture_dir(dir("europeana")).unwrap();
    let page = conn.discover(&subject_columbus(), None).await.unwrap();
    assert_eq!(page.documents.len(), 2);
}

#[test]
fn bnf_notice_uses_ark() {
    let raw = std::fs::read_to_string(dir("bnf").join("details/cb11985936k.json")).unwrap();
    let detail: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let n = normalize_bnf_notice(&detail).unwrap();
    assert_eq!(n.source_kind, SourceKind::Bnf);
    assert_eq!(n.external_id, "ark:/12148/cb11985936k");
    assert!(n
        .identifiers
        .iter()
        .any(|i| i.value_normalized.contains("12148")));
}

#[tokio::test]
async fn bnf_discover_from_fixture() {
    let conn = BnfConnector::from_fixture_dir(dir("bnf")).unwrap();
    let page = conn.discover(&subject_columbus(), None).await.unwrap();
    assert_eq!(page.documents.len(), 2);
    let fetched = conn.fetch(&page.documents[0]).await.unwrap();
    assert!(fetched.text.to_lowercase().contains("colomb"));
}

#[tokio::test]
async fn registry_marks_heritage_connectors_implemented() {
    let corpus = CorpusConnectors {
        internet_archive: Some(
            InternetArchiveConnector::from_fixture_dir(dir("internet_archive")).unwrap(),
        ),
        europeana: Some(EuropeanaConnector::from_fixture_dir(dir("europeana")).unwrap()),
        bnf: Some(BnfConnector::from_fixture_dir(dir("bnf")).unwrap()),
        ..CorpusConnectors::default()
    };
    let reg = talaria_sources::connectors::default_registry_with_corpus(None, false, corpus).unwrap();
    assert!(reg.get(&SourceKind::InternetArchive).unwrap().implemented);
    assert!(reg.get(&SourceKind::Europeana).unwrap().implemented);
    assert!(reg.get(&SourceKind::Bnf).unwrap().implemented);
    assert!(!reg.get(&SourceKind::Gallica).unwrap().implemented);
}

#[test]
fn europeana_live_without_key_is_not_configured() {
    let err = EuropeanaConnector::new(talaria_sources::connectors::EuropeanaConfig {
        api_key: None,
        ..talaria_sources::connectors::EuropeanaConfig::default()
    });
    assert!(err.is_err());
}
