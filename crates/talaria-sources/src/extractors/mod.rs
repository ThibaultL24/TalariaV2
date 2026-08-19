// crates/talaria-sources/src/extractors/mod.rs
//! Multi-strategy extractors → EventCandidate fields (never write canonical events).

mod claim;
mod dense;
mod itinerary;
mod military;
mod posthumous;
mod publication;
mod structured;
mod timeline;
mod travel;

pub use claim::{claim_fingerprint, ClaimKey};
pub use dense::DenseClauseExtractor;
pub use itinerary::ItineraryExtractor;
pub use military::MilitaryCampaignExtractor;
pub use posthumous::PosthumousEventExtractor;
pub use publication::PublicationExtractor;
pub use structured::StructuredStatementExtractor;
pub use timeline::TimelineListExtractor;
pub use travel::TravelResidenceExtractor;

use talaria_quality::{ClauseAnalyzeInput, ClauseExtraction};

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
}

impl ExtractorInput {
    pub fn effective_subject(&self) -> String {
        self.subject_label
            .clone()
            .or_else(|| self.page_title.clone())
            .unwrap_or_else(|| "Unknown".into())
    }
}

pub fn default_extractor_stack() -> Vec<Box<dyn CandidateExtractor>> {
    extractor_stack_for_classes(&[crate::PersonClass::Unknown], false)
}

pub fn extractor_stack_for(class: crate::PersonClass) -> Vec<Box<dyn CandidateExtractor>> {
    extractor_stack_for_classes(&[class], class == crate::PersonClass::MilitaryLeader)
}

pub fn extractor_stack_for_classes(
    classes: &[crate::PersonClass],
    has_military_signal: bool,
) -> Vec<Box<dyn CandidateExtractor>> {
    let enable_military = has_military_signal
        || classes
            .iter()
            .any(|c| *c == crate::PersonClass::MilitaryLeader);
    let mut stack: Vec<Box<dyn CandidateExtractor>> = vec![
        Box::new(StructuredStatementExtractor),
        Box::new(TimelineListExtractor),
    ];
    if enable_military {
        stack.push(Box::new(MilitaryCampaignExtractor));
    }
    stack.extend([
        Box::new(ItineraryExtractor) as Box<dyn CandidateExtractor>,
        Box::new(DenseClauseExtractor),
        Box::new(TravelResidenceExtractor),
        Box::new(PublicationExtractor),
        Box::new(PosthumousEventExtractor),
    ]);
    stack
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
