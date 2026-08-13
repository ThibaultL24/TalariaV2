// crates/talaria-sources/tests/lot_e_density.rs
//! Lot E density / occurrence / place tests (no network).

use talaria_quality::{
    competing_places, occurrence_key, occurrence_stem_for_event, parse_typed_time, TypedTime,
};
use talaria_sources::extractors::{default_extractor_stack, ExtractorInput};
use talaria_sources::{
    is_plausible_place_label, place_hint_from_title, resolve_place_offline, DensityProgress,
    DensityTargets,
};

#[test]
fn arrival_and_departure_are_distinct_occurrences() {
    let t = parse_typed_time(Some("1815-03-07"));
    let a = occurrence_key(
        "Napoleon",
        "arrival",
        "arrived_in",
        &t,
        Some("Grenoble"),
        None,
        None,
        None,
    );
    let b = occurrence_key(
        "Napoleon",
        "departure",
        "departed_for",
        &t,
        Some("Grenoble"),
        None,
        None,
        None,
    );
    assert_ne!(a, b);
}

#[test]
fn same_battle_same_occurrence_key() {
    let t = parse_typed_time(Some("1815-06-18"));
    let a = occurrence_key(
        "Napoleon",
        "battle",
        "fought_at",
        &t,
        Some("Waterloo"),
        Some("Battle of Waterloo"),
        None,
        None,
    );
    let b = occurrence_key(
        "Napoleon",
        "battle",
        "fought_at",
        &t,
        Some("Waterloo"),
        Some("Battle of Waterloo"),
        None,
        None,
    );
    assert_eq!(a, b);
}

#[test]
fn distinct_battles_same_day_differ() {
    let t = parse_typed_time(Some("1815-06-16"));
    let a = occurrence_key(
        "Napoleon",
        "battle",
        "fought_at",
        &t,
        Some("Ligny"),
        Some("Battle of Ligny"),
        None,
        None,
    );
    let b = occurrence_key(
        "Napoleon",
        "battle",
        "fought_at",
        &t,
        Some("Quatre Bras"),
        Some("Battle of Quatre Bras"),
        None,
        None,
    );
    assert_ne!(a, b);
}

#[test]
fn two_extractors_must_not_split_occurrence() {
    let t = parse_typed_time(Some("1815-06-18"));
    // Historical key ignores extractor — same fact from dense vs military = one occurrence.
    let dense = occurrence_key(
        "Napoleon",
        "battle",
        "fought_at",
        &t,
        Some("Waterloo"),
        Some("Battle of Waterloo"),
        None,
        None,
    );
    let military = occurrence_key(
        "Napoleon",
        "battle",
        "fought_at",
        &t,
        Some("Waterloo"),
        Some("Battle of Waterloo"),
        None,
        None,
    );
    assert_eq!(dense, military);
    let with_extractor = occurrence_key(
        "Napoleon",
        "battle",
        "fought_at",
        &t,
        Some("Waterloo"),
        Some("Battle of Waterloo"),
        Some("dense_clause"),
        None,
    );
    assert_ne!(dense, with_extractor);
}

#[test]
fn competing_places_share_stem_not_occurrence() {
    let t = parse_typed_time(Some("1814-05-18"));
    let stem_paris = occurrence_stem_for_event("Napoleon", "residence", "located_at", &t, None);
    let stem_font = occurrence_stem_for_event("Napoleon", "residence", "located_at", &t, None);
    assert_eq!(stem_paris, stem_font);
    let occ_paris = occurrence_key(
        "Napoleon",
        "residence",
        "located_at",
        &t,
        Some("Paris"),
        None,
        None,
        None,
    );
    let occ_font = occurrence_key(
        "Napoleon",
        "residence",
        "located_at",
        &t,
        Some("Fontainebleau"),
        None,
        None,
        None,
    );
    assert_ne!(occ_paris, occ_font);
    assert!(competing_places(Some("Paris"), &[Some("Fontainebleau")]).is_some());
}

#[test]
fn year_precision_not_forced_to_january_first() {
    let t = parse_typed_time(Some("1796"));
    match t {
        TypedTime::Exact {
            year,
            month,
            day,
            ..
        } => {
            assert_eq!(year, 1796);
            assert!(month.is_none());
            assert!(day.is_none());
        }
        TypedTime::Approx { year, .. } => assert_eq!(year, 1796),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn historical_alias_resolves() {
    let r = resolve_place_offline("Austerlitz").expect("austerlitz");
    assert!((r.lat - 49.15).abs() < 0.1);
    let r2 = resolve_place_offline("Jena–Auerstedt").expect("jena compound");
    assert!(r2.lat > 50.0);
}

#[test]
fn place_plausibility_filters_noise() {
    assert!(is_plausible_place_label("Waterloo"));
    assert!(!is_plausible_place_label("November"));
    assert!(!is_plausible_place_label("his youth"));
}

#[test]
fn military_page_yields_battle_candidate() {
    let stack = default_extractor_stack();
    let input = ExtractorInput {
        text: "The Battle of Waterloo was fought on 18 June 1815 near Waterloo.".into(),
        page_title: Some("Battle of Waterloo".into()),
        subject_label: Some("Napoleon".into()),
        document_type: "article".into(),
        subject_death_year: Some(1821),
    };
    let mut raws = Vec::new();
    for ex in &stack {
        raws.extend(ex.extract(&input));
    }
    assert!(raws.iter().any(|r| r.event_type == "battle"));
    assert_eq!(
        place_hint_from_title("Battle of Waterloo").as_deref(),
        Some("Waterloo")
    );
}

#[test]
fn itinerary_steps_are_separate() {
    let stack = default_extractor_stack();
    let input = ExtractorInput {
        text: "In 1815 Napoleon departed for Cannes.\nHe arrived in Grenoble in March 1815.\nHe arrived in Paris in 1815.".into(),
        page_title: Some("Hundred Days".into()),
        subject_label: Some("Napoleon".into()),
        document_type: "article".into(),
        subject_death_year: Some(1821),
    };
    let mut raws = Vec::new();
    for ex in &stack {
        raws.extend(ex.extract(&input));
    }
    let itin: Vec<_> = raws
        .iter()
        .filter(|r| r.extractor_id == "itinerary")
        .collect();
    assert!(
        itin.len() >= 2,
        "expected multiple itinerary steps, got {:?}",
        itin
    );
}

#[test]
fn density_targets_signal_not_reached() {
    let targets = DensityTargets::default();
    assert_eq!(targets.target_map_events, 500);
    let progress = DensityProgress {
        timeline_events: 100,
        map_events: 50,
        documents_processed: 10,
        target_reached: false,
        status: String::new(),
    }
    .evaluate(&targets);
    assert!(!progress.target_reached);
    assert_eq!(progress.status, "target_not_reached");
}
