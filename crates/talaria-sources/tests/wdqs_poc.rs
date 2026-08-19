// crates/talaria-sources/tests/wdqs_poc.rs
use std::path::PathBuf;

use talaria_sources::extractors::{CandidateExtractor, ExtractorInput, StructuredStatementExtractor};
use talaria_sources::wdqs::{
    events_from_fixture_dir, events_to_statement_text, merge_events_for_person,
    parse_sparql_bindings,
};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/wdqs")
}

fn load(name: &str) -> serde_json::Value {
    let raw = std::fs::read_to_string(fixture_dir().join(name)).unwrap();
    serde_json::from_str(&raw).unwrap()
}

#[test]
fn parse_drops_undated_and_keeps_battle_coords() {
    let events = parse_sparql_bindings(&load("participant.json"), None);
    assert_eq!(events.len(), 2);
    let austerlitz = events.iter().find(|e| e.event_qid == "Q179250").unwrap();
    assert_eq!(austerlitz.label, "Battle of Austerlitz");
    assert_eq!(austerlitz.date, "1805-12-02");
    assert_eq!(austerlitz.place_label.as_deref(), Some("Austerlitz"));
    assert_eq!(austerlitz.event_type, "battle");
    assert!((austerlitz.lat.unwrap() - 49.1281).abs() < 0.001);
    assert!((austerlitz.lon.unwrap() - 16.7622).abs() < 0.001);
    let tilsit = events.iter().find(|e| e.event_qid == "Q6882").unwrap();
    assert_eq!(tilsit.event_type, "diplomatic");
}

#[test]
fn merge_dedupes_by_qid_and_unions_p710_p1344_p607() {
    let p710 = parse_sparql_bindings(&load("participant.json"), None);
    let p1344 = parse_sparql_bindings(&load("participant_in.json"), None);
    let wars = parse_sparql_bindings(&load("battles.json"), Some("battle"));
    let merged = merge_events_for_person(&p710, &p1344, &wars);
    let qids: Vec<_> = merged.iter().map(|e| e.event_qid.as_str()).collect();
    assert_eq!(qids, ["Q179250", "Q6882", "Q151851", "Q48314"]);
    let waterloo = merged.iter().find(|e| e.event_qid == "Q48314").unwrap();
    assert_eq!(waterloo.event_type, "battle");
    assert!((waterloo.lat.unwrap() - 50.68).abs() < 0.01);
}

#[test]
fn statements_carry_object_year_and_coords_for_extractor() {
    let events = events_from_fixture_dir(&fixture_dir()).unwrap();
    let text = events_to_statement_text(&events);
    assert!(text.contains("STATEMENT\tbattle\tfought_at\t1805\tAusterlitz\tBattle of Austerlitz"));
    let ex = StructuredStatementExtractor;
    let raws = ex.extract(&ExtractorInput {
        text,
        page_title: Some("Napoleon".into()),
        subject_label: Some("Napoleon".into()),
        document_type: "structured_statement".into(),
        subject_death_year: Some(1821),
    });
    let aust = raws
        .iter()
        .find(|r| r.object_surface.as_deref() == Some("Battle of Austerlitz"))
        .unwrap();
    assert_eq!(aust.event_type, "battle");
    assert_eq!(aust.time_surface.as_deref(), Some("1805"));
    assert_eq!(aust.place_surface.as_deref(), Some("Austerlitz"));
    assert!(aust.lat.is_some() && aust.lon.is_some());
}
