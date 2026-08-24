// crates/talaria-sources/tests/wikisource.rs
use serde_json::Value;
use talaria_sources::connectors::{
    parse_fetch_page, WikisourceConnector, normalize_wikisource,
};
use talaria_sources::{AcademicStatus, AccessLevel, DocumentType, SourceKind};

fn index_and_main_fixture() -> Value {
    serde_json::json!({
        "query": {
            "pages": {
                "101": {
                    "pageid": 101,
                    "ns": 106,
                    "title": "Index:Correspondance.djvu",
                    "lastrevid": 1,
                    "revisions": [{
                        "revid": 1,
                        "slots": {"main": {"content": "{{Index page}}\n{{PR|0}}" }}
                    }]
                },
                "102": {
                    "pageid": 102,
                    "ns": 104,
                    "title": "Page:Correspondance.djvu/1",
                    "lastrevid": 2,
                    "revisions": [{
                        "revid": 2,
                        "slots": {"main": {"content": "{{PageQuality|problematic}}\n{{PR}}" }}
                    }]
                },
                "103": {
                    "pageid": 103,
                    "ns": 114,
                    "title": "Livre:Correspondance",
                    "lastrevid": 3,
                    "revisions": [{
                        "revid": 3,
                        "slots": {"main": {"content": "Livre wrapper" }}
                    }]
                },
                "42": {
                    "pageid": 42,
                    "ns": 0,
                    "title": "Correspondance de Napoléon Ier",
                    "lastrevid": 99,
                    "pageprops": {"wikibase_item": "Q123"},
                    "revisions": [{
                        "revid": 99,
                        "slots": {"main": {"content": "Lettre du 15 août 1805 à Joséphine." }}
                    }]
                }
            }
        }
    })
}

fn is_skipped_title(title: &str) -> bool {
    let folded = title.trim().to_ascii_lowercase();
    folded.starts_with("index:")
        || folded.starts_with("page:")
        || folded.starts_with("livre:")
}

fn pages_as_singletons(json: &Value) -> Vec<(String, String, Value)> {
    let Some(pages) = json.pointer("/query/pages").and_then(Value::as_object) else {
        return Vec::new();
    };
    pages
        .values()
        .filter_map(|page| {
            let title = page.get("title").and_then(Value::as_str)?.to_string();
            let singleton = serde_json::json!({"query": {"pages": {"x": page.clone()}}});
            let (text, meta) = parse_fetch_page(&singleton)?;
            Some((title, text, meta))
        })
        .collect()
}

#[test]
fn wikisource_index_and_main_fixture_normalizes_only_main_work_as_primary_source() {
    let fixture = index_and_main_fixture();
    let normalized: Vec<_> = pages_as_singletons(&fixture)
        .into_iter()
        .filter(|(title, _, _)| !is_skipped_title(title))
        .map(|(title, text, meta)| {
            let doc = WikisourceConnector::document_from_title(&title);
            normalize_wikisource(&doc, &text, &meta).unwrap()
        })
        .collect();

    assert_eq!(normalized.len(), 1);
    let n = &normalized[0];
    assert_eq!(n.source_kind, SourceKind::Wikisource);
    assert_eq!(n.academic_status, AcademicStatus::PrimarySource);
    assert_eq!(n.academic_status.as_str(), "primary_source");
    assert_eq!(n.external_id, "42");
    assert_eq!(n.title, "Correspondance de Napoléon Ier");
    assert_eq!(n.connector_version, "wikisource:fr_v1");
    assert_eq!(n.access_level, AccessLevel::Open);
    assert!(n.full_text_available);
    assert_eq!(n.snapshot_text, "Lettre du 15 août 1805 à Joséphine.");
    assert_eq!(n.raw_metadata["wiki"], "frwikisource");
    assert_eq!(n.raw_metadata["genre"], "letter");
}

#[test]
fn wikisource_letter_title_normalizes_as_correspondence_primary_source() {
    let doc = WikisourceConnector::document_from_title("Lettre à Joséphine");
    let n = normalize_wikisource(&doc, "Ma chère Joséphine,", &serde_json::json!({}))
        .unwrap();
    assert_eq!(n.document_type, DocumentType::Correspondence);
    assert_eq!(n.academic_status, AcademicStatus::PrimarySource);
    assert_eq!(n.external_id, "Lettre à Joséphine");
    assert!(n.full_text_available);
}

#[test]
fn wikisource_empty_wikitext_is_not_full_text() {
    let doc = WikisourceConnector::document_from_title("Histoire de France");
    let n = normalize_wikisource(&doc, "   \n", &serde_json::json!({"page_id": 7})).unwrap();
    assert!(!n.full_text_available);
    assert_eq!(n.external_id, "7");
    assert_eq!(n.document_type, DocumentType::Other("narrative".into()));
}

#[test]
fn wikisource_proofread_problematic_is_recorded() {
    let doc = WikisourceConnector::document_from_title("Discours aux états généraux");
    let n = normalize_wikisource(
        &doc,
        "{{PR|problematic}}\nProofreadPage quality",
        &serde_json::json!({"page_id": 9}),
    )
    .unwrap();
    assert_eq!(n.raw_metadata["proofread_level"], "problematic");
}
