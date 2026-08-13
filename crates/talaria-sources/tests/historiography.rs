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
