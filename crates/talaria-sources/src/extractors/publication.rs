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
        for (i, unit) in crate::extractors::split_prose_units(&input.text).into_iter().enumerate() {
            let line = unit.as_str();
            let lower = line.to_lowercase();
            if !(lower.contains("published")
                || lower.contains("wrote ")
                || lower.contains("authored")
                || lower.contains("printed")
                || lower.contains("publia")
                || lower.contains("publié")
                || lower.contains("publie")
                || lower.contains("parut")
                || lower.contains("parution")
                || lower.contains("édita")
                || lower.contains("edita"))
            {
                continue;
            }
            let Some(year) = find_year(line) else {
                continue;
            };
            out.push(RawCandidate {
                event_type: "publication".into(),
                predicate: "published".into(),
                subject_surface: subject.clone(),
                time_surface: Some(year),
                place_surface: super::travel::find_place(line),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractors::ExtractorInput;

    #[test]
    fn french_publication_verbs_yield_a_dated_work() {
        let raws = PublicationExtractor.extract(&ExtractorInput {
            text: "Baudelaire publia Les Fleurs du mal à Paris en 1857.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert_eq!(raws.len(), 1);
        assert_eq!(raws[0].event_type, "publication");
        assert_eq!(raws[0].time_surface.as_deref(), Some("1857"));
        assert_eq!(raws[0].place_surface.as_deref(), Some("Paris"));
    }
}
