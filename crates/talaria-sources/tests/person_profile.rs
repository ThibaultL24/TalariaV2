// crates/talaria-sources/tests/person_profile.rs
use talaria_sources::extractors::{extractor_stack_for, ExtractorInput};
use talaria_sources::{
    catalog_search_query, infer_person_class, plan_sources, profile_for, rank_wikipedia_title,
    IngestBudgets, PersonClass, ResolvedSubject, SourceKind,
};

#[test]
fn physicist_is_scientist_and_denies_battles() {
    let class = infer_person_class(&["physicist".into(), "chemist".into()], None);
    assert_eq!(class, PersonClass::Scientist);
    let p = profile_for(class);
    assert!(!p.enable_wdqs_military);
    assert!(!p.enable_military_extractor);
    assert!(rank_wikipedia_title("University of Paris", &p, None) > 0.5);
    assert!(rank_wikipedia_title("Battle of Waterloo", &p, None) < 0.4);
}

#[test]
fn writer_denies_coalition_pages() {
    let class = infer_person_class(&["writer".into()], Some("French poet and novelist"));
    assert_eq!(class, PersonClass::ArtistWriter);
    let p = profile_for(class);
    assert!(rank_wikipedia_title("Les Misérables", &p, None) > 0.5);
    assert!(rank_wikipedia_title("Battle of Waterloo", &p, None) < 0.4);
}

#[test]
fn explorer_beats_admiral_military_qid() {
    let class = infer_person_class(&["explorer".into(), "military".into()], None);
    assert_eq!(class, PersonClass::Explorer);
    let p = profile_for(class);
    assert!(!p.enable_wdqs_military);
    assert!(rank_wikipedia_title("Voyages of Christopher Columbus", &p, None) > 0.5);
}

#[test]
fn napoleon_stays_military_leader() {
    let class = infer_person_class(&["military".into(), "statesman".into()], None);
    assert_eq!(class, PersonClass::MilitaryLeader);
    let p = profile_for(class);
    assert!(p.enable_wdqs_military);
    assert!(p.enable_military_extractor);
    assert!(rank_wikipedia_title("Battle of Austerlitz", &p, None) > 0.5);
}

#[test]
fn wartime_scientist_stays_scientist() {
    let class = infer_person_class(
        &["computer scientist".into(), "cryptanalyst".into()],
        Some("British mathematician and computer scientist"),
    );
    assert_eq!(class, PersonClass::Scientist);
}

#[test]
fn ruler_from_pharaoh_lead() {
    let class = infer_person_class(&["civilian".into()], Some("Queen of Ptolemaic Egypt"));
    assert_eq!(class, PersonClass::Ruler);
}

#[test]
fn scientist_catalog_queries_are_typed() {
    let p = profile_for(PersonClass::Scientist);
    let bnf = catalog_search_query("Marie Curie", &p, SourceKind::Bnf);
    assert!(bnf.contains("Marie Curie"));
    assert!(bnf.to_lowercase().contains("laborato") || bnf.to_lowercase().contains("nobel"));
    let oa = catalog_search_query("Marie Curie", &p, SourceKind::OpenAlex);
    assert!(oa.contains("Marie Curie"));
    assert!(!oa.contains("origins OR historiography"));
}

#[test]
fn explorer_europeana_asks_voyage_not_only_controversy() {
    let p = profile_for(PersonClass::Explorer);
    let q = catalog_search_query("Christophe Colomb", &p, SourceKind::Europeana);
    assert!(q.contains("Christophe Colomb"));
    let low = q.to_lowercase();
    assert!(low.contains("voyage") || low.contains("expedition") || low.contains("navigation"));
}

#[test]
fn plan_scientist_prioritizes_openalex_not_battle_wiki() {
    let subject = ResolvedSubject {
        entity_id: None,
        qid: Some("Q7186".into()),
        label: "Marie Curie".into(),
        languages: vec!["fr".into(), "en".into()],
        birth_year: Some(1867),
        death_year: Some(1934),
        countries: vec!["France".into(), "Poland".into()],
        occupations: vec!["scientist".into()],
        known_identifiers: vec![],
    };
    let plan = plan_sources(&subject, IngestBudgets::default());
    assert!(plan.sources.iter().any(|s| s.kind == SourceKind::OpenAlex));
    assert!(plan.sources.iter().any(|s| s.kind == SourceKind::Bnf));
    assert!(plan.sources.iter().any(|s| s.kind == SourceKind::Europeana));
    assert!(!plan.sources.iter().any(|s| {
        s.kind == SourceKind::Wikipedia && s.reason.contains("battle")
    }));
    assert_eq!(plan.subject.person_class(), PersonClass::Scientist);
}

#[test]
fn scientist_extractor_stack_omits_military_campaign() {
    let stack = extractor_stack_for(PersonClass::Scientist);
    assert!(!stack.iter().any(|e| e.extractor_id() == "military_campaign"));
    let input = ExtractorInput {
        text: "She published in 1903.".into(),
        page_title: Some("Marie Curie".into()),
        subject_label: Some("Marie Curie".into()),
        document_type: "article".into(),
        subject_death_year: Some(1934),
    };
    let pubs: Vec<_> = stack
        .iter()
        .flat_map(|e| e.extract(&input))
        .filter(|c| c.event_type == "publication")
        .collect();
    assert!(!pubs.is_empty());
}

#[test]
fn scientist_wiki_filter_drops_battles_keeps_labs() {
    let p = profile_for(PersonClass::Scientist);
    let kept = talaria_sources::filter_wiki_titles_for_profile(
        "Marie Curie",
        vec![
            "Marie Curie".into(),
            "Battle of Waterloo".into(),
            "University of Paris".into(),
        ],
        &p,
        Some(1934),
    );
    assert_eq!(kept[0], "Marie Curie");
    assert!(kept.iter().any(|t| t.contains("University")));
    assert!(!kept.iter().any(|t| t.contains("Waterloo")));
}
