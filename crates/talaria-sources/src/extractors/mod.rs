// crates/talaria-sources/src/extractors/mod.rs
//! Multi-strategy extractors → EventCandidate fields (never write canonical events).

mod claim;
mod dense;
mod infobox;
mod itinerary;
mod keywords;
mod military;
mod posthumous;
mod publication;
mod structured;
mod timeline;
mod travel;

pub use claim::{claim_fingerprint, ClaimKey};
pub use dense::DenseClauseExtractor;
pub use infobox::InfoboxExtractor;
pub use itinerary::{is_country_or_region, ItineraryExtractor};
pub use keywords::KeywordMineExtractor;
pub use military::MilitaryCampaignExtractor;
pub use posthumous::PosthumousEventExtractor;
pub use publication::PublicationExtractor;
pub use structured::StructuredStatementExtractor;
pub use timeline::TimelineListExtractor;
pub use travel::TravelResidenceExtractor;

use talaria_quality::{ClauseAnalyzeInput, ClauseExtraction};

/// Split wiki prose so one publication verb cannot swallow a travel sentence.
pub(crate) fn split_prose_units(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if ch == '\n' || ch == '.' || ch == '!' || ch == '?' {
            let t = cur.trim().trim_end_matches(['.', '!', '?']).trim();
            if !t.is_empty() {
                out.push(t.to_string());
            }
            cur.clear();
        }
    }
    let t = cur.trim().to_string();
    if !t.is_empty() {
        out.push(t);
    }
    out
}

#[derive(Debug, Clone)]
pub struct RawCandidate {
    pub event_type: String,
    pub predicate: String,
    pub subject_surface: String,
    pub time_surface: Option<String>,
    pub place_surface: Option<String>,
    pub object_surface: Option<String>,
    pub participant_surfaces: Vec<String>,
    pub clause_text: String,
    pub clause_index: i32,
    pub start_offset: i32,
    pub end_offset: i32,
    pub cross_clause_join: bool,
    pub extractor_id: String,
    pub is_posthumous: bool,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

pub trait CandidateExtractor: Send + Sync {
    fn extractor_id(&self) -> &str;
    fn version(&self) -> &str;
    fn extract(&self, input: &ExtractorInput) -> Vec<RawCandidate>;
}

#[derive(Debug, Clone)]
pub struct ExtractorInput {
    pub text: String,
    pub page_title: Option<String>,
    pub subject_label: Option<String>,
    pub document_type: String,
    pub subject_death_year: Option<i32>,
    /// MediaWiki source when the fetch kept templates and links.
    pub wikitext: Option<String>,
    /// Place labels already marked as wikilinks on the page.
    pub known_places: Vec<String>,
}

impl Default for ExtractorInput {
    fn default() -> Self {
        Self {
            text: String::new(),
            page_title: None,
            subject_label: None,
            document_type: "article".into(),
            subject_death_year: None,
            wikitext: None,
            known_places: vec![],
        }
    }
}

impl ExtractorInput {
    pub fn effective_subject(&self) -> String {
        self.subject_label
            .clone()
            .or_else(|| self.page_title.clone())
            .unwrap_or_else(|| "Unknown".into())
    }
}

/// The subject's own biography page — every dated clause on it is about them.
pub fn page_is_subject_biography(page_title: &str, subject: &str) -> bool {
    let page = page_title.split('(').next().unwrap_or(page_title).trim();
    let subject = subject.split('(').next().unwrap_or(subject).trim();
    if page.is_empty() || subject.is_empty() {
        return false;
    }
    let page_l = page.to_lowercase();
    let subject_l = subject.to_lowercase();
    page_l == subject_l || page_l.starts_with(&subject_l)
}

fn clause_names_subject(clause: &str, subject: &str) -> bool {
    let subject = subject.split('(').next().unwrap_or(subject).trim();
    if subject.is_empty() {
        return false;
    }
    let lower = clause.to_lowercase();
    let subject_l = subject.to_lowercase();
    if lower.contains(&subject_l) {
        return true;
    }
    if let Some(sur) = crate::seeds::subject_surname(subject) {
        if lower.contains(&sur.to_lowercase()) {
            return true;
        }
    }
    false
}

/// Keep extract on the bio page and on place/battle pages we chose to follow.
/// Birth/death come only from infobox / Wikidata — never from another bio.
pub fn keep_extracted_raw(raw: &RawCandidate, page_title: &str, subject: &str) -> bool {
    if matches!(raw.event_type.as_str(), "birth" | "death") {
        return matches!(
            raw.extractor_id.as_str(),
            "infobox" | "structured_statement"
        );
    }
    if raw.extractor_id == "infobox" || raw.extractor_id == "structured_statement" {
        return true;
    }
    if !clause_is_about_subject(&raw.clause_text, subject) {
        return false;
    }
    if page_is_subject_biography(page_title, subject) {
        return true;
    }
    clause_names_subject(&raw.clause_text, subject)
}

/// Drop clauses whose grammatical agent is another named person.
pub fn clause_is_about_subject(clause: &str, subject: &str) -> bool {
    let subject = subject.split('(').next().unwrap_or(subject).trim();
    if subject.is_empty() {
        return true;
    }
    let lower = clause.to_lowercase();
    let subject_l = subject.to_lowercase();
    if lower.contains(&subject_l) {
        return true;
    }
    if let Some(sur) = crate::seeds::subject_surname(subject) {
        if lower.contains(&sur.to_lowercase()) {
            return true;
        }
    }
    if let Some(agent) = leading_person_agent(clause) {
        let agent_l = agent.to_lowercase();
        if subject_l.contains(&agent_l) || agent_l.contains(&subject_l) {
            return true;
        }
        if let Some(sur) = crate::seeds::subject_surname(subject) {
            if agent_l.contains(&sur.to_lowercase()) {
                return true;
            }
        }
        return false;
    }
    true
}

fn leading_person_agent(clause: &str) -> Option<String> {
    const SKIP: &[&str] = &[
        "en", "in", "le", "la", "les", "the", "on", "after", "during", "puis", "alors", "dès",
        "des", "de", "du", "au", "aux", "un", "une", "a", "an", "il", "elle", "ils", "elles",
        "he", "she", "they", "his", "her", "their",
        "january", "february", "march", "april", "may", "june", "july", "august", "september",
        "october", "november", "december", "janvier", "février", "fevrier", "mars", "avril",
        "mai", "juin", "juillet", "août", "aout", "septembre", "octobre", "novembre", "décembre",
        "decembre",
    ];
    let mut words = Vec::new();
    for w in clause.split_whitespace() {
        let clean = w.trim_matches(|c: char| !c.is_alphabetic() && c != '-');
        if clean.is_empty() {
            continue;
        }
        let lower = clean.to_lowercase();
        if words.is_empty() && (SKIP.contains(&lower.as_str()) || clean.chars().all(|c| c.is_ascii_digit()))
        {
            continue;
        }
        let first = clean.chars().next()?;
        if first.is_uppercase() {
            words.push(clean.to_string());
            if words.len() >= 3 {
                break;
            }
            continue;
        }
        break;
    }
    if words.len() >= 2 {
        Some(words.join(" "))
    } else if words.len() == 1 && words[0].chars().count() >= 3 {
        Some(words[0].clone())
    } else {
        None
    }
}

#[cfg(test)]
mod subject_clause_tests {
    use super::clause_is_about_subject;

    #[test]
    fn keeps_pronoun_and_subject_name() {
        assert!(clause_is_about_subject(
            "En 1833 elle partit pour Venise.",
            "George Sand"
        ));
        assert!(clause_is_about_subject(
            "George Sand s'installe à Nohant en 1831.",
            "George Sand"
        ));
    }

    #[test]
    fn drops_other_named_person_birth() {
        assert!(!clause_is_about_subject(
            "Victor Hugo was born in Besançon in 1802.",
            "George Sand"
        ));
        assert!(!clause_is_about_subject(
            "Le 7 novembre 1659, les Espagnols acceptent de signer le traité des Pyrénées.",
            "Louis XIV",
        ));
    }

    #[test]
    fn drops_other_person_on_their_bio_page() {
        let raw = super::RawCandidate {
            event_type: "residence".into(),
            predicate: "resided_in".into(),
            subject_surface: "Marie Curie".into(),
            time_surface: Some("1920".into()),
            place_surface: Some("Dublin".into()),
            object_surface: None,
            participant_surfaces: vec![],
            clause_text: "On 6 April 1920, Schrödinger married Annemarie Bertel in Vienna.".into(),
            clause_index: 72,
            start_offset: 0,
            end_offset: 80,
            cross_clause_join: false,
            extractor_id: "travel_residence".into(),
            is_posthumous: false,
            lat: None,
            lon: None,
        };
        assert!(!super::keep_extracted_raw(
            &raw,
            "Erwin Schrödinger",
            "Marie Curie"
        ));
    }

    #[test]
    fn drops_nobel_list_clause_about_other_laureate() {
        let raw = super::RawCandidate {
            event_type: "award".into(),
            predicate: "received".into(),
            subject_surface: "Marie Curie".into(),
            time_surface: Some("1933".into()),
            place_surface: Some("Stockholm".into()),
            object_surface: None,
            participant_surfaces: vec![],
            clause_text: "Erwin Schrödinger received the Nobel Prize in Physics in 1933.".into(),
            clause_index: 1,
            start_offset: 0,
            end_offset: 70,
            cross_clause_join: false,
            extractor_id: "dense_clause".into(),
            is_posthumous: false,
            lat: None,
            lon: None,
        };
        assert!(!super::keep_extracted_raw(
            &raw,
            "Nobel Prize in Physics",
            "Marie Curie"
        ));
    }

    #[test]
    fn bio_page_drops_third_party_dated_clause() {
        let raw = super::RawCandidate {
            event_type: "diplomatic".into(),
            predicate: "signed".into(),
            subject_surface: "Louis XIV".into(),
            time_surface: Some("1659".into()),
            place_surface: Some("Pyrénées".into()),
            object_surface: None,
            participant_surfaces: vec![],
            clause_text: "Le 7 novembre 1659, les Espagnols acceptent de signer le traité des Pyrénées."
                .into(),
            clause_index: 0,
            start_offset: 0,
            end_offset: 80,
            cross_clause_join: false,
            extractor_id: "dump_keywords".into(),
            is_posthumous: false,
            lat: None,
            lon: None,
        };
        assert!(!super::keep_extracted_raw(&raw, "Louis XIV", "Louis XIV"));
        assert!(!super::keep_extracted_raw(&raw, "Traité des Pyrénées", "Louis XIV"));
        assert!(!super::keep_extracted_raw(&raw, "Anne d'Autriche", "Louis XIV"));
        assert!(!super::keep_extracted_raw(
            &super::RawCandidate {
                event_type: "death".into(),
                extractor_id: "dump_keywords".into(),
                ..raw.clone()
            },
            "Louis XIV",
            "Louis XIV"
        ));
    }

    #[test]
    fn wiki_life_stack_always_includes_military() {
        assert!(super::default_extractor_stack()
            .iter()
            .any(|e| e.extractor_id() == "military_campaign"));
    }
}

pub fn default_extractor_stack() -> Vec<Box<dyn CandidateExtractor>> {
    vec![
        Box::new(InfoboxExtractor),
        Box::new(StructuredStatementExtractor),
        Box::new(TimelineListExtractor),
        Box::new(MilitaryCampaignExtractor),
        Box::new(ItineraryExtractor),
        Box::new(DenseClauseExtractor),
        Box::new(KeywordMineExtractor),
        Box::new(TravelResidenceExtractor),
        Box::new(PublicationExtractor),
        Box::new(PosthumousEventExtractor),
    ]
}

pub fn extractor_stack_for(_class: crate::PersonClass) -> Vec<Box<dyn CandidateExtractor>> {
    default_extractor_stack()
}

pub fn extractor_stack_for_classes(
    _classes: &[crate::PersonClass],
    _has_military_signal: bool,
) -> Vec<Box<dyn CandidateExtractor>> {
    default_extractor_stack()
}

pub fn to_clause_extraction(raw: &RawCandidate) -> ClauseExtraction {
    ClauseExtraction {
        clause_index: raw.clause_index,
        clause_text: raw.clause_text.clone(),
        clause_start_offset: raw.start_offset,
        clause_end_offset: raw.end_offset,
        subject_surface: raw.subject_surface.clone(),
        event_type: raw.event_type.clone(),
        predicate: raw.predicate.clone(),
        time_surface: raw.time_surface.clone(),
        place_surface: raw.place_surface.clone(),
        object_surface: raw.object_surface.clone(),
        participant_surfaces: raw.participant_surfaces.clone(),
        cross_clause_join: raw.cross_clause_join,
    }
}

pub fn analyze_as_clause_input(text: &str, title: Option<&str>) -> ClauseAnalyzeInput {
    ClauseAnalyzeInput {
        text: text.to_string(),
        page_title: title.map(str::to_string),
        start_offset: 0,
    }
}
