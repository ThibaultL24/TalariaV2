// crates/talaria-sources/src/extractors/structured.rs
use crate::extractors::{CandidateExtractor, ExtractorInput, RawCandidate};

/// Wikidata / STATEMENT-line extractor.
pub struct StructuredStatementExtractor;

impl CandidateExtractor for StructuredStatementExtractor {
    fn extractor_id(&self) -> &str {
        "structured_statement"
    }

    fn version(&self) -> &str {
        "structured:v1"
    }

    fn extract(&self, input: &ExtractorInput) -> Vec<RawCandidate> {
        if input.document_type != "structured_statement"
            && !input.text.lines().any(|l| l.starts_with("STATEMENT\t"))
        {
            return vec![];
        }
        let subject = input
            .page_title
            .clone()
            .unwrap_or_else(|| "Unknown".into());
        let mut out = Vec::new();
        for (i, line) in input.text.lines().enumerate() {
            let line = line.trim();
            if !line.starts_with("STATEMENT\t") {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            // STATEMENT event_type predicate year place
            if parts.len() < 5 {
                continue;
            }
            let event_type = parts[1].to_string();
            let predicate = parts[2].to_string();
            let time = if parts[3].is_empty() {
                None
            } else {
                Some(parts[3].to_string())
            };
            let place = if parts[4].is_empty() {
                None
            } else {
                Some(parts[4].to_string())
            };
            let start = input.text[..input.text.find(line).unwrap_or(0)].len() as i32;
            out.push(RawCandidate {
                event_type,
                predicate,
                subject_surface: subject.clone(),
                time_surface: time,
                place_surface: place,
                object_surface: None,
                participant_surfaces: vec![],
                clause_text: line.to_string(),
                clause_index: i as i32,
                start_offset: start,
                end_offset: start + line.len() as i32,
                cross_clause_join: false,
                extractor_id: self.extractor_id().into(),
                is_posthumous: false,
            });
        }
        out
    }
}
