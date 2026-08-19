// crates/talaria-sources/tests/theses_fr_corpus.rs
use std::path::PathBuf;

use talaria_sources::connectors::ThesesFrConnector;
use talaria_sources::{
    match_subject_to_document, normalize_identifier, normalize_these_detail, AcademicStatus,
    AccessLevel, DiscoveryCursor, DocumentType, IdentifierScheme, ResolvedSubject, SourceConnector,
    SourceKind,
};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/theses_fr")
}

fn subject_napoleon() -> ResolvedSubject {
    ResolvedSubject {
        entity_id: None,
        qid: Some("Q517".into()),
        label: "Napoleon".into(),
        languages: vec!["fr".into()],
        birth_year: Some(1769),
        death_year: Some(1821),
        countries: vec!["France".into()],
        occupations: vec![],
        known_identifiers: vec![],
    }
}

#[tokio::test]
async fn theses_fr_discover_paginates_fixtures() {
    let conn = ThesesFrConnector::from_fixture_dir(fixture_dir()).unwrap();
    let subject = subject_napoleon();
    let page1 = conn.discover(&subject, None).await.unwrap();
    // default page_size 20 > 3 fixtures → single page
    assert_eq!(page1.documents.len(), 3);
    assert!(page1.next_cursor.is_none());

    let cfg = talaria_sources::connectors::ThesesFrConfig {
        fixture_dir: Some(fixture_dir()),
        page_size: 2,
        ..talaria_sources::connectors::ThesesFrConfig::default()
    };
    let small = ThesesFrConnector::new(cfg).unwrap();
    let p1 = small.discover(&subject, None).await.unwrap();
    assert_eq!(p1.documents.len(), 2);
    assert!(p1.next_cursor.is_some());
    let p2 = small.discover(&subject, p1.next_cursor).await.unwrap();
    assert_eq!(p2.documents.len(), 1);
    assert!(p2.next_cursor.is_none());
}

#[tokio::test]
async fn theses_fr_fetch_normalizes_defended_and_in_progress() {
    let conn = ThesesFrConnector::from_fixture_dir(fixture_dir()).unwrap();
    let subject = subject_napoleon();
    let page = conn.discover(&subject, None).await.unwrap();
    let defended = page
        .documents
        .iter()
        .find(|d| d.external_id == "2020AIXM0123")
        .unwrap();
    let fetched = conn.fetch(defended).await.unwrap();
    let n = fetched.raw_metadata.get("normalized").cloned().unwrap();
    let n: talaria_sources::NormalizedCorpusDocument = serde_json::from_value(n).unwrap();
    assert_eq!(n.academic_status, AcademicStatus::DoctoralDefended);
    assert!(n.full_text_available);
    assert_eq!(n.access_level, AccessLevel::Open);
    assert_eq!(n.document_type, DocumentType::Thesis);
    assert!(n
        .identifiers
        .iter()
        .any(|i| i.scheme == IdentifierScheme::Nnt && i.value_normalized == "2020AIXM0123"));
    assert!(n
        .identifiers
        .iter()
        .any(|i| i.scheme == IdentifierScheme::Doi));
    assert!(n
        .contributions
        .iter()
        .any(|c| c.role == talaria_sources::ContributionRole::Author
            && c.identifier_value.as_deref() == Some("123456789")));
    assert!(n
        .contributions
        .iter()
        .any(|c| c.role == talaria_sources::ContributionRole::ThesisAdvisor));
    assert!(n
        .contributions
        .iter()
        .any(|c| c.role == talaria_sources::ContributionRole::Institution));
    assert!(n.subjects.iter().any(|s| s.scheme == "rameau"));

    let in_prep = page
        .documents
        .iter()
        .find(|d| d.external_id == "s98765")
        .unwrap();
    let fetched2 = conn.fetch(in_prep).await.unwrap();
    let n2: talaria_sources::NormalizedCorpusDocument =
        serde_json::from_value(fetched2.raw_metadata.get("normalized").cloned().unwrap()).unwrap();
    assert_eq!(n2.academic_status, AcademicStatus::AcademicUnreviewed);
    assert!(!n2.full_text_available);
    assert_eq!(n2.access_level, AccessLevel::MetadataOnly);
    assert_eq!(n2.document_type, DocumentType::BibliographicNotice);
}

#[tokio::test]
async fn theses_fr_idempotent_fetch_same_revision() {
    let conn = ThesesFrConnector::from_fixture_dir(fixture_dir()).unwrap();
    let subject = subject_napoleon();
    let page = conn.discover(&subject, None).await.unwrap();
    let doc = &page.documents[0];
    let a = conn.fetch(doc).await.unwrap();
    let b = conn.fetch(doc).await.unwrap();
    assert_eq!(a.text, b.text);
    assert_eq!(a.revision_id, b.revision_id);
}

#[test]
fn notice_is_not_an_event_and_match_is_explicable() {
    let raw = std::fs::read_to_string(fixture_dir().join("details/2020AIXM0123.json")).unwrap();
    let detail: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let n = normalize_these_detail(&detail).unwrap();
    let m = match_subject_to_document("Napoleon", &n).unwrap();
    assert_eq!(m.relation, "about");
    assert_eq!(m.match_version, talaria_sources::SUBJECT_MATCH_V1);
    assert!(m
        .components
        .iter()
        .any(|c| c.key == "title_overlap" || c.key == "rameau_hit" || c.key == "abstract_overlap"));

    let chem = std::fs::read_to_string(fixture_dir().join("details/2015PA010999.json")).unwrap();
    let chem: serde_json::Value = serde_json::from_str(&chem).unwrap();
    let n2 = normalize_these_detail(&chem).unwrap();
    assert!(match_subject_to_document("Napoleon", &n2).is_none());
}

#[test]
fn identifier_normalization_nnt_doi_ppn() {
    assert_eq!(
        normalize_identifier(IdentifierScheme::Nnt, "2020aixm0123").as_deref(),
        Some("2020AIXM0123")
    );
    assert_eq!(
        normalize_identifier(IdentifierScheme::Doi, "https://doi.org/10.1/X").as_deref(),
        Some("10.1/x")
    );
    assert_eq!(
        normalize_identifier(IdentifierScheme::Ppn, "026403715").as_deref(),
        Some("026403715")
    );
}

#[test]
fn status_axes_are_independent() {
    let defended_open = normalize_these_detail(&serde_json::json!({
        "titrePrincipal": "Napoléon et l'État",
        "nnt": "2020TEST0001",
        "status": "soutenue",
        "isSoutenue": true,
        "accessible": "oui",
        "langues": ["fr"],
        "auteurs": [],
        "directeurs": [],
        "resumes": { "fr": "Sur Napoléon." },
        "mapSujets": {}
    }))
    .unwrap();
    assert_eq!(
        defended_open.academic_status,
        AcademicStatus::DoctoralDefended
    );
    assert_eq!(defended_open.document_type, DocumentType::Thesis);
    assert!(defended_open.full_text_available);
    assert_eq!(defended_open.access_level, AccessLevel::Open);

    let defended_notice = normalize_these_detail(&serde_json::json!({
        "titrePrincipal": "Napoléon sans texte",
        "nnt": "2020TEST0002",
        "status": "soutenue",
        "isSoutenue": true,
        "accessible": "non",
        "langues": ["fr"],
        "auteurs": [],
        "directeurs": [],
        "resumes": {},
        "mapSujets": {}
    }))
    .unwrap();
    assert_eq!(
        defended_notice.academic_status,
        AcademicStatus::DoctoralDefended
    );
    assert_eq!(defended_notice.document_type, DocumentType::Thesis);
    assert!(!defended_notice.full_text_available);
    assert_eq!(defended_notice.access_level, AccessLevel::MetadataOnly);

    let in_prep = normalize_these_detail(&serde_json::json!({
        "titrePrincipal": "Projet Napoléon",
        "numSujet": "s1",
        "status": "enCours",
        "isSoutenue": false,
        "accessible": "non",
        "langues": ["fr"],
        "auteurs": [],
        "directeurs": [],
        "resumes": {},
        "mapSujets": {}
    }))
    .unwrap();
    assert_eq!(in_prep.academic_status, AcademicStatus::AcademicUnreviewed);
    assert_eq!(in_prep.document_type, DocumentType::BibliographicNotice);
    assert!(!in_prep.full_text_available);
    assert_eq!(in_prep.access_level, AccessLevel::MetadataOnly);
}

#[test]
fn multiple_identifiers_coexist_without_ambiguity() {
    let n = normalize_these_detail(&serde_json::json!({
        "titrePrincipal": "Multi-id",
        "nnt": "2020TEST0003",
        "doi": "10.1234/Abc",
        "numSujet": "s42",
        "status": "soutenue",
        "isSoutenue": true,
        "accessible": "non",
        "auteurs": [],
        "directeurs": [],
        "mapSujets": {}
    }))
    .unwrap();
    let schemes: Vec<_> = n.identifiers.iter().map(|i| i.scheme).collect();
    assert!(schemes.contains(&IdentifierScheme::Nnt));
    assert!(schemes.contains(&IdentifierScheme::Doi));
    assert!(schemes.contains(&IdentifierScheme::NumSujet));
    assert_eq!(
        n.identifiers
            .iter()
            .find(|i| i.scheme == IdentifierScheme::Doi)
            .unwrap()
            .value_normalized,
        "10.1234/abc"
    );
}

#[test]
fn metadata_change_creates_new_fingerprint_without_touching_axes() {
    let mut detail = serde_json::json!({
        "titrePrincipal": "Napoléon A",
        "nnt": "2020TEST0004",
        "status": "soutenue",
        "isSoutenue": true,
        "accessible": "non",
        "auteurs": [{ "nom": "Dupont", "prenom": "A", "ppn": "1" }],
        "directeurs": [],
        "resumes": { "fr": "Version 1" },
        "mapSujets": {}
    });
    let v1 = normalize_these_detail(&detail).unwrap();
    detail["resumes"]["fr"] = serde_json::json!("Version 2 — texte modifié");
    let v2 = normalize_these_detail(&detail).unwrap();
    assert_ne!(v1.content_fingerprint(), v2.content_fingerprint());
    assert_ne!(v1.revision_token, v2.revision_token);
    // Old axes unchanged by abstract edit.
    assert_eq!(v1.academic_status, v2.academic_status);
    assert_eq!(v1.access_level, v2.access_level);
    assert_eq!(v1.full_text_available, v2.full_text_available);
}

#[test]
fn subject_match_v1_keeps_versioned_components() {
    let raw = std::fs::read_to_string(fixture_dir().join("details/2020AIXM0123.json")).unwrap();
    let detail: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let n = normalize_these_detail(&detail).unwrap();
    let m = match_subject_to_document("Napoleon", &n).unwrap();
    assert_eq!(m.match_version, talaria_sources::SUBJECT_MATCH_V1);
    assert!(!m.components.is_empty());
    assert!(m
        .components
        .iter()
        .all(|c| !c.key.is_empty() && c.weight > 0.0));
    assert!(m.evidence_summary.contains("title_overlap") || m.evidence_summary.contains("rameau"));
}

#[test]
fn keyset_cursor_format_is_stable() {
    // Mirrors API encode/parse: {score:.6}_{uuid}
    let id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    let enc = format!("{:.6}_{id}", 0.75_f32);
    let mut parts = enc.splitn(2, '_');
    let score: f32 = parts.next().unwrap().parse().unwrap();
    let parsed = parts.next().unwrap();
    assert!((score - 0.75).abs() < 1e-5);
    assert_eq!(parsed, id);
    assert_eq!(500_i64.clamp(1, 200), 200);
}

#[tokio::test]
async fn registry_lists_theses_fr() {
    let conn = ThesesFrConnector::from_fixture_dir(fixture_dir()).unwrap();
    let reg =
        talaria_sources::connectors::default_registry_with_theses(None, false, Some(conn)).unwrap();
    let entry = reg.get(&SourceKind::ThesesFr).unwrap();
    assert!(entry.implemented);
    assert_eq!(entry.kind, SourceKind::ThesesFr);
    let _ = DiscoveryCursor::default();
}
