// crates/talaria-sources/tests/historiography.rs
use talaria_sources::historiography::{
    is_historiography_section, scan_bibliographic, scan_passage, DebateType, EvidenceLayer,
    EventHint,
};

#[test]
fn death_section_is_historiographic_early_life_is_not() {
    assert!(is_historiography_section("Death and burial"));
    assert!(is_historiography_section("Historiography"));
    assert!(is_historiography_section("Postérité et légende"));
    assert!(!is_historiography_section("Early life"));
    assert!(!is_historiography_section("Italian campaign"));
}

#[test]
fn arsenic_theory_is_cause_of_death_not_a_map_fact() {
    let hits = scan_passage(
        "Some historians have hypothesized that Napoleon was poisoned with arsenic on Saint Helena. \
         The orthodox view is that he died of stomach cancer.",
    );
    assert!(hits
        .iter()
        .any(|h| h.debate_type == DebateType::CauseOfDeathDispute
            && h.evidence_layer == EvidenceLayer::TheoryOrLegend
            && h.claim_kind == "theory"
            && h.event_hint == Some(EventHint::Death)));
}

#[test]
fn archival_gap_is_evidence_layer() {
    let hits = scan_passage(
        "No contemporary source records the conversation; the archives were destroyed in 1814.",
    );
    assert_eq!(hits[0].debate_type, DebateType::ArchivalGap);
    assert_eq!(hits[0].evidence_layer, EvidenceLayer::EvidenceGap);
}

#[test]
fn historians_debate_is_interpretation() {
    let hits = scan_passage(
        "Historians debate whether the 18 Brumaire was a coup d'état or a restoration of order.",
    );
    assert_eq!(hits[0].debate_type, DebateType::InterpretationDispute);
    assert_eq!(hits[0].evidence_layer, EvidenceLayer::Interpretation);
    assert_eq!(hits[0].claim_kind, "debate_stance");
}

#[test]
fn birth_fact_sentence_is_not_a_debate() {
    let hits = scan_passage("Napoleon Bonaparte was born in 1769 in Ajaccio.");
    assert!(hits.is_empty());
}

#[test]
fn theses_title_relectures_is_a_debate_candidate() {
    let hits = scan_bibliographic("Relectures historiographiques de Napoléon", None);
    assert!(!hits.is_empty());
    assert_eq!(hits[0].evidence_layer, EvidenceLayer::Interpretation);
}

#[test]
fn polymer_chemistry_thesis_is_ignored() {
    let hits = scan_bibliographic("La chimie des polymères au XXIe siècle", None);
    assert!(hits.is_empty());
}

#[test]
fn columbus_origin_debate_is_identity_not_a_birth_fact() {
    let hits = scan_passage(include_str!(
        "../../../fixtures/historiography/columbus_origins.txt"
    ));
    assert!(
        hits.iter()
            .any(|h| h.debate_type == DebateType::IdentityOriginDispute
                && h.claim_kind == "theory"
                && h.event_hint == Some(EventHint::Birth)),
        "expected identity_origin_dispute theory, got {hits:?}"
    );
    assert!(
        hits.iter().any(|h| h.debate_type == DebateType::IdentityOriginDispute
            && (h.evidence_layer == EvidenceLayer::EvidenceGap
                || h.quote.to_lowercase().contains("correspondance"))),
        "expected correspondence as origin evidence, got {hits:?}"
    );
}

#[test]
fn columbus_birth_year_alone_is_not_an_origin_debate() {
    let hits = scan_passage("Christopher Columbus was born in 1451 in Genoa.");
    assert!(hits.is_empty());
}

#[test]
fn origin_section_title_is_historiographic() {
    assert!(is_historiography_section("Origins and identity"));
    assert!(is_historiography_section("Origines"));
    assert!(is_historiography_section("Early life and origins"));
}

#[test]
fn columbus_origins_title_is_identity_theory() {
    let hits = scan_bibliographic("Les origines de Christophe Colomb", None);
    assert_eq!(hits[0].debate_type, DebateType::IdentityOriginDispute);
    assert_eq!(hits[0].claim_kind, "theory");
    assert_eq!(hits[0].event_hint, Some(EventHint::Birth));
}

#[test]
fn persee_birth_date_title_is_chronology_dispute() {
    let hits = scan_bibliographic(
        "Henry Vignaud: The real Birth-Date of Columbus. A critical Study",
        None,
    );
    assert!(!hits.is_empty());
    assert_eq!(hits[0].debate_type, DebateType::ChronologyDispute);
    assert_eq!(hits[0].event_hint, Some(EventHint::Birth));
}

#[test]
fn nationality_title_is_identity_dispute() {
    let hits = scan_bibliographic("Christophe Colomb portugais", None);
    assert_eq!(hits[0].debate_type, DebateType::IdentityOriginDispute);
}

#[test]
fn hero_or_villain_title_is_interpretation() {
    let hits = scan_bibliographic(
        "Elite Revisionists and Popular Beliefs: Christopher Columbus, Hero or Villain?",
        None,
    );
    assert_eq!(hits[0].debate_type, DebateType::InterpretationDispute);
    assert_eq!(hits[0].claim_kind, "debate_stance");
}
