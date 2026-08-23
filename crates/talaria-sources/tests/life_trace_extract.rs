// crates/talaria-sources/tests/life_trace_extract.rs
//! Life-trace extractors must work for any historical person, not one biography.

use talaria_sources::extractors::{default_extractor_stack, ExtractorInput};
use talaria_sources::resolve_place_offline;

fn extract(subject: &str, text: &str, death: Option<i32>) -> Vec<talaria_sources::extractors::RawCandidate> {
    let input = ExtractorInput {
        text: text.into(),
        page_title: Some(subject.into()),
        subject_label: Some(subject.into()),
        document_type: "article".into(),
        subject_death_year: death,
        ..Default::default()
    };
    let stack = default_extractor_stack();
    let mut raws = Vec::new();
    for ex in &stack {
        raws.extend(ex.extract(&input));
    }
    raws
}

#[test]
fn french_paragraph_yields_travel_and_publication_separately() {
    let raws = extract(
        "George Sand",
        "En décembre 1833, George Sand partit pour Venise. Elle publia Lélia à Paris en 1833.",
        Some(1876),
    );
    assert!(
        raws.iter().any(|r| r.event_type == "departure"
            && r.place_surface.as_deref() == Some("Venise")
            && r.time_surface.as_deref() == Some("1833")),
        "missing Venice departure: {raws:?}"
    );
    assert!(
        raws.iter().any(|r| r.event_type == "publication"
            && r.time_surface.as_deref() == Some("1833")),
        "missing publication: {raws:?}"
    );
}

#[test]
fn scientist_residence_and_meeting_are_typed() {
    let raws = extract(
        "Marie Curie",
        "Marie Curie s'installe à Paris en 1891. Elle rencontre Pierre Curie à Paris en 1894.",
        Some(1934),
    );
    assert!(raws.iter().any(|r| r.event_type == "residence"
        && r.place_surface.as_deref() == Some("Paris")
        && r.time_surface.as_deref() == Some("1891")));
    assert!(raws.iter().any(|r| r.event_type == "meeting"
        && r.place_surface.as_deref() == Some("Paris")
        && r.time_surface.as_deref() == Some("1894")));
}

#[test]
fn artist_english_arrival_geocodes_offline() {
    let raws = extract(
        "Leonardo da Vinci",
        "In 1482 Leonardo arrived in Milan.",
        Some(1519),
    );
    let arrival = raws
        .iter()
        .find(|r| r.event_type == "arrival")
        .expect("arrival");
    assert_eq!(arrival.place_surface.as_deref(), Some("Milan"));
    assert!(resolve_place_offline("Milan").is_some());
}

#[test]
fn french_exonyms_resolve_without_person_rules() {
    for label in [
        "Venise",
        "Majorque",
        "Marseille",
        "Nohant",
        "Place des Vosges",
        "Varsovie",
        "Gênes",
    ] {
        assert!(
            resolve_place_offline(label).is_some(),
            "offline gazetteer must resolve {label} for any biography"
        );
    }
}

#[test]
fn itinerary_chain_and_infobox_are_generic() {
    let raws = extract(
        "George Sand",
        "Ils partent de Paris le 12 décembre 1833, s'embarquent à Marseille, débarquent à Gênes, visitent Florence et parviennent à Venise le 31 décembre.",
        Some(1876),
    );
    for place in ["Marseille", "Gênes", "Florence", "Venise"] {
        assert!(
            raws.iter().any(|r| r.place_surface.as_deref() == Some(place)),
            "missing {place}: {raws:?}"
        );
    }

    let infobox = r#"{{Infobox writer
| birth_date  = {{birth date|1804|7|1}}
| birth_place = [[Paris]]
| death_date  = {{death date and age|1876|6|8|1804|7|1}}
| death_place = [[Nohant-Vic]]
}}"#;
    let input = ExtractorInput {
        text: String::new(),
        page_title: Some("George Sand".into()),
        subject_label: Some("George Sand".into()),
        document_type: "article".into(),
        subject_death_year: Some(1876),
        wikitext: Some(infobox.into()),
        known_places: vec![],
    };
    let stack = default_extractor_stack();
    let mut raws = Vec::new();
    for ex in &stack {
        raws.extend(ex.extract(&input));
    }
    assert!(raws.iter().any(|r| r.event_type == "birth"
        && r.place_surface.as_deref() == Some("Paris")
        && r.time_surface.as_deref() == Some("1804")));
    assert!(raws.iter().any(|r| r.event_type == "death"
        && r.place_surface.as_deref() == Some("Nohant-Vic")
        && r.time_surface.as_deref() == Some("1876")));
}

#[test]
fn other_person_life_events_are_not_attributed_to_the_subject() {
    use talaria_sources::extractors::clause_is_about_subject;
    assert!(!clause_is_about_subject(
        "Victor Hugo was born in Besançon in 1802.",
        "George Sand"
    ));
    let raws = extract(
        "George Sand",
        "Victor Hugo was born in Besançon in 1802. Victor Hugo s'installe à Hauteville House en 1855.",
        Some(1876),
    );
    let kept: Vec<_> = raws
        .into_iter()
        .filter(|r| clause_is_about_subject(&r.clause_text, "George Sand"))
        .collect();
    assert!(
        !kept.iter().any(|r| r.place_surface.as_deref() == Some("Besançon")
            || r.clause_text.to_lowercase().contains("hauteville")),
        "other-person facts leaked: {kept:?}"
    );
}

#[test]
fn dump_keywords_mine_any_searched_person() {
    let raws = extract(
        "Ada Lovelace",
        "Ada Lovelace was born in 1815 in London and later worked with Babbage.",
        Some(1852),
    );
    assert!(
        raws.iter().any(|r| r.extractor_id == "dump_keywords"
            && r.time_surface.as_deref() == Some("1815")
            && r.place_surface.as_deref().is_some_and(|p| p.to_lowercase().contains("london"))),
        "generic keyword mine missed Ada: {raws:?}"
    );
}

#[test]
fn french_royal_bio_sentences_yield_dated_places() {
    let raws = extract(
        "Louis XIV",
        "Louis XIV est sacré le 7 juin 1654 en la cathédrale de Reims.\n\
         À partir de 1682, Louis XIV dirige son royaume depuis le château de Versailles.\n\
         Le 7 novembre 1659, Louis XIV signe le traité des Pyrénées.",
        Some(1715),
    );
    assert!(
        raws.iter().any(|r| r.time_surface.as_deref() == Some("1654")
            && r.place_surface.as_deref().is_some_and(|p| p.contains("Reims"))),
        "missing Reims coronation: {raws:?}"
    );
    assert!(
        raws.iter().any(|r| r.time_surface.as_deref() == Some("1682")
            && r.place_surface.as_deref().is_some_and(|p| p.contains("Versailles"))),
        "missing Versailles residence: {raws:?}"
    );
    assert!(
        raws.iter().any(|r| r.event_type == "diplomatic"
            && r.time_surface.as_deref() == Some("1659")),
        "missing Pyrenees treaty: {raws:?}"
    );
}

#[test]
fn year_and_place_without_verb_still_yield_a_point() {
    let raws = extract(
        "Louis XIV",
        "À partir de 1682 la cour demeure au château de Versailles pour tout le règne.",
        Some(1715),
    );
    assert!(
        raws.iter().any(|r| r.time_surface.as_deref() == Some("1682")
            && r.place_surface.as_deref().is_some_and(|p| p.to_lowercase().contains("versailles"))),
        "year+place without verb missed: {raws:?}"
    );
}
