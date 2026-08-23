// crates/talaria-sources/tests/lot_c_catalog.rs
use talaria_sources::connectors::{
    normalize_europeana_item, normalize_ia_item, GallicaConnector, OpenLibraryConnector,
};
use talaria_sources::extractors::{default_extractor_stack, ExtractorInput};
use talaria_sources::ResolvedSubject;

fn napoleon() -> ResolvedSubject {
    ResolvedSubject {
        entity_id: None,
        qid: Some("Q517".into()),
        label: "Napoleon".into(),
        languages: vec!["fr".into(), "en".into()],
        birth_year: Some(1769),
        death_year: Some(1821),
        countries: vec!["France".into()],
        occupations: vec!["military".into()],
        known_identifiers: vec![],
    }
}

#[test]
fn open_library_authored_notice_stays_in_life_window() {
    let payload = serde_json::json!({
        "docs": [
            {
                "key": "/works/OL1W",
                "title": "Mémoires",
                "author_name": ["Napoleon Bonaparte"],
                "first_publish_year": 1823,
                "publish_place": ["Paris"]
            },
            {
                "key": "/works/OL2W",
                "title": "A modern biography",
                "author_name": ["Jane Historian"],
                "first_publish_year": 2015,
                "publish_place": ["London"]
            }
        ]
    });
    let docs = OpenLibraryConnector::parse_search(&napoleon(), &payload);
    assert_eq!(docs.len(), 2);
    let authored = docs.iter().find(|d| d.external_id == "/works/OL1W").unwrap();
    let notice = authored.source_metadata.raw["notice"].as_str().unwrap();
    assert!(notice.contains("published"));
    assert!(notice.contains("1823"));
    assert!(notice.contains("Paris"));
    let about = docs.iter().find(|d| d.external_id == "/works/OL2W").unwrap();
    let about_notice = about.source_metadata.raw["notice"].as_str().unwrap();
    assert!(!about_notice.contains("Napoleon published"));
}

#[test]
fn open_library_drops_authored_work_outside_lifespan() {
    let payload = serde_json::json!({
        "docs": [{
            "key": "/works/OL9W",
            "title": "Spurious",
            "author_name": ["Napoleon"],
            "first_publish_year": 1999,
            "publish_place": ["Paris"]
        }]
    });
    let docs = OpenLibraryConnector::parse_search(&napoleon(), &payload);
    assert!(docs.is_empty());
}

#[test]
fn internet_archive_normalizes_item() {
    let detail = serde_json::json!({
        "metadata": {
            "identifier": "memorialsthelena",
            "title": "Memorial of Saint Helena",
            "date": "1823",
            "creator": "Napoleon",
            "description": "Dictated on Saint Helena."
        }
    });
    let doc = normalize_ia_item(&detail).expect("normalize_ia_item should succeed");
    assert!(doc.title.contains("Saint Helena"));
}

#[test]
fn europeana_normalizes_item() {
    let item = serde_json::json!({
        "id": "/123/abc",
        "title": ["Portrait of Napoleon"],
        "year": ["1804"],
        "dcCreator": ["Jacques-Louis David"],
        "dcDescription": ["Napoleon crowned in Paris in 1804."],
        "edmIsShownAt": ["https://example.org/item"]
    });
    let doc = normalize_europeana_item(&item).expect("normalize_europeana_item should succeed");
    assert!(doc.title.contains("Napoleon") || doc.raw_metadata.to_string().contains("1804"));
}

#[test]
fn gallica_parses_sru_records() {
    let xml = r#"
    <srw:searchRetrieveResponse>
      <srw:record>
        <dc:title>Le Mémorial de Sainte-Hélène</dc:title>
        <dc:creator>Napoléon Bonaparte</dc:creator>
        <dc:date>1823</dc:date>
        <dc:coverage>Paris</dc:coverage>
        <dc:identifier>https://gallica.bnf.fr/ark:/12148/bpt6k123</dc:identifier>
      </srw:record>
    </srw:searchRetrieveResponse>
    "#;
    let mut subject = napoleon();
    subject.label = "Napoléon Bonaparte".into();
    let docs = GallicaConnector::parse_sru(&subject, xml);
    assert_eq!(docs.len(), 1);
    let notice = docs[0].source_metadata.raw["notice"].as_str().unwrap();
    assert!(notice.contains("published"));
    assert!(notice.contains("Paris"));
}

#[test]
fn publication_extractor_reads_year_and_place() {
    let stack = default_extractor_stack();
    let input = ExtractorInput {
        text: "Napoleon published \"Mémoires\" in 1823 in Paris.".into(),
        page_title: Some("Napoleon".into()),
        subject_label: Some("Napoleon".into()),
        document_type: "bibliographic_notice".into(),
        subject_death_year: Some(1821),
        ..Default::default()
    };
    let mut found = false;
    for extractor in &stack {
        for cand in extractor.extract(&input) {
            if cand.event_type == "publication" {
                assert_eq!(cand.time_surface.as_deref(), Some("1823"));
                assert_eq!(cand.place_surface.as_deref(), Some("Paris"));
                found = true;
            }
        }
    }
    assert!(found);
}

#[test]
fn live_registry_implements_search_catalogs() {
    let reg = talaria_sources::connectors::default_registry(None, true)
        .expect("live registry should build without network");
    for kind in [
        talaria_sources::SourceKind::Hal,
        talaria_sources::SourceKind::Persee,
        talaria_sources::SourceKind::Gallica,
        talaria_sources::SourceKind::ThesesFr,
        talaria_sources::SourceKind::OpenAlex,
        talaria_sources::SourceKind::Bnf,
        talaria_sources::SourceKind::OpenLibrary,
        talaria_sources::SourceKind::InternetArchive,
    ] {
        let entry = reg.get(&kind).unwrap_or_else(|| panic!("missing {}", kind.as_str()));
        assert!(
            entry.implemented,
            "{} should be implemented in live registry",
            kind.as_str()
        );
    }
}
