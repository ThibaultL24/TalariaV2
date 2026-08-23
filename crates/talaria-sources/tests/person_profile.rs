// crates/talaria-sources/tests/person_profile.rs
use talaria_sources::extractors::{extractor_stack_for, extractor_stack_for_classes, ExtractorInput};
use talaria_sources::{
    catalog_search_buckets, catalog_search_query, infer_person_class, infer_person_classes,
    keep_military_typed_event, plan_sources, profile_for, rank_wikipedia_title, IngestBudgets,
    PersonClass, ResolvedSubject, SourceKind,
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
    assert!(rank_wikipedia_title("Bibliography of Victor Hugo", &p, None) > 0.5);
    assert!(rank_wikipedia_title("Battle of Waterloo", &p, None) < 0.4);
    assert!(rank_wikipedia_title("Maison de George Sand", &p, None) >= 0.55);
    assert!(rank_wikipedia_title("Un hiver à Majorque", &p, None) >= 0.55);
    let kept = talaria_sources::filter_wiki_titles_for_classes(
        "George Sand",
        vec![
            "George Sand".into(),
            "Un hiver à Majorque".into(),
            "Maison de George Sand".into(),
            "List of Belgian football clubs".into(),
        ],
        &[class],
        Some(1876),
        false,
    );
    assert!(kept.iter().any(|t| t.contains("Majorque")));
    assert!(kept.iter().any(|t| t.contains("Maison")));
    assert!(!kept.iter().any(|t| t.contains("football")));
}

#[test]
fn explorer_plus_military_keeps_both_facets() {
    let classes = infer_person_classes(&["explorer".into(), "military".into()], None);
    assert!(classes.contains(&PersonClass::Explorer));
    assert!(classes.contains(&PersonClass::MilitaryLeader));
    let stack = extractor_stack_for_classes(&classes, true);
    assert!(stack.iter().any(|e| e.extractor_id() == "military_campaign"));
    assert!(
        rank_wikipedia_title(
            "Battle of the Atlantic",
            &profile_for(PersonClass::MilitaryLeader),
            None
        ) >= 0.55
    );
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
fn explorer_hal_uses_solr_profile_fields() {
    let p = profile_for(PersonClass::Explorer);
    let q = catalog_search_query("Christophe Colomb", &p, SourceKind::Hal);
    assert!(q.contains("text:\"Christophe Colomb\""));
    let low = q.to_lowercase();
    assert!(low.contains("title_t:voyage") || low.contains("keyword_s:navigation"));
}

#[test]
fn explorer_gallica_uses_valid_cql() {
    let p = profile_for(PersonClass::Explorer);
    let q = catalog_search_query("Christophe Colomb", &p, SourceKind::Gallica);
    assert!(q.contains("gallica all \"Christophe Colomb\""));
    assert!(q.contains("dc.subject all"));
}

#[test]
fn explorer_persee_buckets_add_profile_terms() {
    let p = profile_for(PersonClass::Explorer);
    let buckets = catalog_search_buckets("Christophe Colomb", &p, SourceKind::Persee);
    assert!(buckets.iter().any(|b| b == "Christophe Colomb"));
    assert!(buckets.iter().any(|b| b.contains("voyage")));
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
fn scientist_extractor_stack_still_extracts_publications() {
    let stack = extractor_stack_for(PersonClass::Scientist);
    assert!(stack.iter().any(|e| e.extractor_id() == "military_campaign"));
    let input = ExtractorInput {
        text: "She published in 1903.".into(),
        page_title: Some("Marie Curie".into()),
        subject_label: Some("Marie Curie".into()),
        document_type: "article".into(),
        subject_death_year: Some(1934),
        ..Default::default()
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

#[test]
fn writer_soldier_keeps_military_career() {
    let classes = infer_person_classes(&["writer".into(), "soldier".into()], None);
    assert!(classes.contains(&PersonClass::ArtistWriter));
    assert!(classes.contains(&PersonClass::MilitaryLeader));
    assert!(keep_military_typed_event(
        "battle",
        "Ernst Jünger fought at the Somme in 1916.",
        "Ernst Jünger",
        true,
    ));
}

#[test]
fn wiki_page_drops_battle_without_person_class() {
    assert!(!keep_military_typed_event(
        "battle",
        "The Battle of Waterloo decided the campaign.",
        "Marie Curie",
        false,
    ));
}

#[test]
fn clause_service_keeps_battle_without_occupation() {
    assert!(keep_military_typed_event(
        "battle",
        "Marie Curie enlisted in 1914 and served as a radiographer.",
        "Marie Curie",
        false,
    ));
}

#[test]
fn default_extractor_stack_includes_military() {
    let stack = talaria_sources::extractors::default_extractor_stack();
    assert!(stack.iter().any(|e| e.extractor_id() == "military_campaign"));
}

#[test]
fn living_ruler_keeps_office_drops_unsourced_battles() {
    let classes = infer_person_classes(&["president".into()], Some("President of France"));
    assert!(classes.contains(&PersonClass::Ruler));
    assert!(!classes.contains(&PersonClass::MilitaryLeader));
    let office = talaria_sources::rank_wikipedia_title_for_classes(
        "French presidential court",
        &classes,
        None,
        false,
    );
    let battle = talaria_sources::rank_wikipedia_title_for_classes(
        "Battle of Waterloo",
        &classes,
        None,
        false,
    );
    assert!(office >= 0.55, "office={office}");
    assert!(battle < 0.55, "battle={battle}");
}

#[test]
fn antiquity_ruler_keeps_bce_diplomatic_not_modern_wars() {
    let classes = infer_person_classes(&["pharaoh".into(), "queen of".into()], Some("Queen of Ptolemaic Egypt"));
    assert!(classes.contains(&PersonClass::Ruler));
    let court = talaria_sources::rank_wikipedia_title_for_classes(
        "Ptolemaic court",
        &classes,
        Some(-30),
        false,
    );
    let ww2 = talaria_sources::rank_wikipedia_title_for_classes(
        "World War II",
        &classes,
        Some(-30),
        false,
    );
    assert!(court >= 0.55, "court={court}");
    assert!(ww2 < 0.55, "ww2={ww2}");
}

#[test]
fn explorer_ranks_voyage_not_world_war() {
    let classes = infer_person_classes(&["explorer".into()], None);
    let voyage = talaria_sources::rank_wikipedia_title_for_classes(
        "Voyages of Christopher Columbus",
        &classes,
        Some(1506),
        false,
    );
    let ww1 = talaria_sources::rank_wikipedia_title_for_classes(
        "World War I",
        &classes,
        Some(1506),
        false,
    );
    assert!(voyage >= 0.55, "voyage={voyage}");
    assert!(ww1 < 0.55, "ww1={ww1}");
}

#[test]
fn scientist_seed_merge_keeps_both_labs_and_battles_from_the_page() {
    let seeds = talaria_sources::merge_seed_titles(
        "Marie Curie",
        ["Marie Curie".into()],
        [
            "Battle of Waterloo".into(),
            "University of Paris".into(),
        ],
        3,
    );
    assert_eq!(seeds[0], "Marie Curie");
    assert!(seeds.iter().any(|t| t.contains("University")));
    assert!(seeds.iter().any(|t| t.contains("Waterloo")));
}

#[test]
fn military_seed_merge_still_keeps_battle_pages() {
    let seeds = talaria_sources::merge_seed_titles_for(
        "Napoleon",
        ["Napoleon".into()],
        ["Battle of Austerlitz".into(), "Comics".into()],
        8,
        true,
    );
    assert!(seeds.iter().any(|t| t.contains("Austerlitz")));
}

#[test]
fn untopical_list_page_drops_below_poc_keep_threshold() {
    let score = talaria_sources::rank_wikipedia_title_for_classes(
        "List of Belgian football clubs",
        &[PersonClass::Scientist],
        Some(1934),
        false,
    );
    assert!(score < 0.55, "score={score}");
}

#[test]
fn polyvalent_artist_scientist_ranks_both_lab_and_atelier() {
    let classes = infer_person_classes(
        &["painter".into(), "scientist".into(), "engineer".into()],
        None,
    );
    assert!(classes.contains(&PersonClass::ArtistVisual));
    assert!(classes.contains(&PersonClass::Scientist));
    let lab = talaria_sources::rank_wikipedia_title_for_classes(
        "University of Florence",
        &classes,
        Some(1519),
        false,
    );
    let atelier = talaria_sources::rank_wikipedia_title_for_classes(
        "Milan atelier",
        &classes,
        Some(1519),
        false,
    );
    assert!(lab >= 0.55, "lab={lab}");
    assert!(atelier >= 0.55, "atelier={atelier}");
    let battle = talaria_sources::rank_wikipedia_title_for_classes(
        "Battle of Waterloo",
        &classes,
        Some(1519),
        false,
    );
    assert!(battle < 0.55, "battle={battle}");
}

#[test]
fn military_leader_still_ranks_battles_but_also_political_career() {
    let p = profile_for(PersonClass::MilitaryLeader);
    assert!(p.enable_wdqs_military);
    let battle = rank_wikipedia_title("Bataille de Montcornet", &p, None);
    let presidence = rank_wikipedia_title("Présidence de Charles de Gaulle", &p, None);
    let gouvernement = rank_wikipedia_title("Gouvernement de Gaulle", &p, None);
    assert!(battle > 0.8);
    assert!(presidence >= 0.55, "presidence={presidence}");
    assert!(gouvernement >= 0.55, "gouvernement={gouvernement}");
}
