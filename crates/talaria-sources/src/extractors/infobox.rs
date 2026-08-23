// crates/talaria-sources/src/extractors/infobox.rs
//! Person infobox → dated birth / death / residence candidates.

use talaria_text::infobox_life_facts;

use crate::extractors::{CandidateExtractor, ExtractorInput, RawCandidate};

pub struct InfoboxExtractor;

impl CandidateExtractor for InfoboxExtractor {
    fn extractor_id(&self) -> &str {
        "infobox"
    }

    fn version(&self) -> &str {
        "infobox:v1"
    }

    fn extract(&self, input: &ExtractorInput) -> Vec<RawCandidate> {
        let src = input.wikitext.as_deref().unwrap_or(input.text.as_str());
        if !src.to_lowercase().contains("{{infobox") {
            return vec![];
        }
        let facts = infobox_life_facts(src);
        let subject = input.effective_subject();
        let mut out = Vec::new();
        push_life(
            &mut out,
            &subject,
            "birth",
            "born_in",
            facts.birth_year,
            facts.birth_place,
            src,
            0,
        );
        push_life(
            &mut out,
            &subject,
            "death",
            "died_in",
            facts.death_year,
            facts.death_place,
            src,
            1,
        );
        out
    }
}

fn push_life(
    out: &mut Vec<RawCandidate>,
    subject: &str,
    event_type: &str,
    predicate: &str,
    year: Option<String>,
    place: Option<String>,
    src: &str,
    index: i32,
) {
    if year.is_none() && place.is_none() {
        return;
    }
    out.push(RawCandidate {
        event_type: event_type.into(),
        predicate: predicate.into(),
        subject_surface: subject.into(),
        time_surface: year,
        place_surface: place,
        object_surface: None,
        participant_surfaces: vec![],
        clause_text: format!("infobox {event_type}"),
        clause_index: index,
        start_offset: 0,
        end_offset: src.len().min(80) as i32,
        cross_clause_join: false,
        extractor_id: "infobox".into(),
        is_posthumous: false,
        lat: None,
        lon: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infobox_source_yields_birth_and_death() {
        let raws = InfoboxExtractor.extract(&ExtractorInput {
            text: String::new(),
            page_title: Some("Marie Curie".into()),
            subject_label: Some("Marie Curie".into()),
            document_type: "article".into(),
            subject_death_year: Some(1934),
            wikitext: Some(
                r#"{{Infobox scientist
| birth_date  = {{birth date|1867|11|7}}
| birth_place = [[Warsaw]]
| death_date  = {{death date and age|1934|7|4|1867|11|7}}
| death_place = [[Passy]]
}}"#
                .into(),
            ),
            known_places: vec![],
        });
        assert!(raws.iter().any(|r| r.event_type == "birth"
            && r.place_surface.as_deref() == Some("Warsaw")
            && r.time_surface.as_deref() == Some("1867")));
        assert!(raws.iter().any(|r| r.event_type == "death"
            && r.place_surface.as_deref() == Some("Passy")
            && r.time_surface.as_deref() == Some("1934")));
    }
}
