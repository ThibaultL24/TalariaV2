// crates/talaria-sources/src/extractors/publication.rs
use crate::extractors::travel::find_year;
use crate::extractors::{CandidateExtractor, ExtractorInput, RawCandidate};

pub struct PublicationExtractor;

impl CandidateExtractor for PublicationExtractor {
    fn extractor_id(&self) -> &str {
        "publication"
    }

    fn version(&self) -> &str {
        "publication:v1"
    }

    fn extract(&self, input: &ExtractorInput) -> Vec<RawCandidate> {
        let subject = input.effective_subject();
        let mut out = Vec::new();
        for (i, line) in input.text.lines().enumerate() {
            let lower = line.to_lowercase();
            if !(lower.contains("published")
                || lower.contains("wrote ")
                || lower.contains("authored"))
            {
                continue;
            }
            out.push(RawCandidate {
                event_type: "publication".into(),
                predicate: "published".into(),
                subject_surface: subject.clone(),
                time_surface: find_year(line),
                place_surface: None,
                object_surface: None,
                participant_surfaces: vec![],
                clause_text: line.trim().to_string(),
                clause_index: i as i32,
                start_offset: 0,
                end_offset: line.len() as i32,
                cross_clause_join: false,
                extractor_id: self.extractor_id().into(),
                is_posthumous: false,
                lat: None,
                lon: None,
            });
        }
        out
    }
}
