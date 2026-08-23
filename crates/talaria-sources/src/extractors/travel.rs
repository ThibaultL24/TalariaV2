// crates/talaria-sources/src/extractors/travel.rs
use crate::extractors::{CandidateExtractor, ExtractorInput, RawCandidate};

pub struct TravelResidenceExtractor;

impl CandidateExtractor for TravelResidenceExtractor {
    fn extractor_id(&self) -> &str {
        "travel_residence"
    }

    fn version(&self) -> &str {
        "travel_residence:v1"
    }

    fn extract(&self, input: &ExtractorInput) -> Vec<RawCandidate> {
        let subject = input.effective_subject();
        let mut out = Vec::new();
        for (i, unit) in crate::extractors::split_prose_units(&input.text).into_iter().enumerate() {
            let line = unit.as_str();
            let lower = line.to_lowercase();
            let (etype, pred) = if lower.contains("departed")
                || lower.contains("left for")
                || lower.contains("partit pour")
                || lower.contains("partent pour")
                || lower.contains("parti pour")
                || lower.contains("partie pour")
            {
                ("departure", "departed_for")
            } else if lower.contains("arrived") || lower.contains("arriva") {
                ("arrival", "arrived_in")
            } else if lower.contains("lived in")
                || lower.contains("resided")
                || lower.contains("vécut")
                || lower.contains("vecut")
                || lower.contains("habita")
                || lower.contains("s'installa")
                || lower.contains("s’installa")
                || lower.contains("s'installe")
                || lower.contains("s’installe")
                || lower.contains("séjourna")
                || lower.contains("sejourna")
                || lower.contains("séjourne")
                || lower.contains("sejourne")
            {
                ("residence", "resided_in")
            } else if lower.contains("stayed in") || lower.contains("stayed at") {
                ("residence", "stayed_at")
            } else if lower.contains("exiled") || lower.contains("s'exila") || lower.contains("exilé") {
                ("exile", "exiled_to")
            } else {
                continue;
            };
            // Avoid double-count with dense on same lines that dense also catches —
            // travel extractor still runs; fingerprint dedupes.
            let year = find_year(line);
            let place = find_place(line);
            out.push(RawCandidate {
                event_type: etype.into(),
                predicate: pred.into(),
                subject_surface: subject.clone(),
                time_surface: year,
                place_surface: place,
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

pub(crate) fn find_year(s: &str) -> Option<String> {
    for w in s.split(|c: char| !c.is_ascii_digit()) {
        if w.len() == 4 {
            if let Ok(y) = w.parse::<i32>() {
                if (1000..=2100).contains(&y) {
                    return Some(y.to_string());
                }
            }
        }
    }
    None
}

pub(crate) fn find_place(s: &str) -> Option<String> {
    let lower = s.to_lowercase();
    for cue in [" in ", " at ", " to ", " for ", " à ", " au ", " aux ", " en ", " pour "] {
        if let Some(pos) = lower.rfind(cue) {
            let after = &s[pos + cue.len()..];
            let raw = after
                .split(|c: char| c == '.' || c.is_ascii_digit() || c == ',')
                .next()?
                .trim()
                .trim_matches(|c: char| !c.is_alphabetic() && c != ' ' && c != '-');
            let token = raw
                .trim_end_matches(" en")
                .trim_end_matches(" in")
                .trim_start_matches("l'")
                .trim_start_matches("l’")
                .trim()
                .to_string();
            if token.len() >= 2 && !token.eq_ignore_ascii_case("en") {
                return Some(token);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractors::ExtractorInput;

    #[test]
    fn french_residence_line_yields_place() {
        let raws = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "Il vécut à Honfleur en 1859.".into(),
            page_title: Some("Charles Baudelaire".into()),
            subject_label: Some("Charles Baudelaire".into()),
            document_type: "article".into(),
            subject_death_year: Some(1867),
            ..Default::default()
        });
        assert_eq!(raws[0].event_type, "residence");
        assert_eq!(raws[0].time_surface.as_deref(), Some("1859"));
        assert_eq!(raws[0].place_surface.as_deref(), Some("Honfleur"));
    }

    #[test]
    fn french_installs_and_stays_are_residences() {
        let raws = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "Elle s'installe à Nohant en 1831. En 1838 elle séjourne à Majorque.".into(),
            page_title: Some("George Sand".into()),
            subject_label: Some("George Sand".into()),
            document_type: "article".into(),
            subject_death_year: Some(1876),
            ..Default::default()
        });
        assert!(
            raws.iter().any(|r| r.place_surface.as_deref() == Some("Nohant")
                && r.time_surface.as_deref() == Some("1831")),
            "{raws:?}"
        );
        assert!(
            raws.iter().any(|r| r.place_surface.as_deref() == Some("Majorque")
                && r.time_surface.as_deref() == Some("1838")),
            "{raws:?}"
        );
    }

    #[test]
    fn travel_sentence_not_swallowed_by_publication_in_same_paragraph() {
        let travel = TravelResidenceExtractor.extract(&ExtractorInput {
            text: "Elle s'installe à Paris en 1831. Elle publia Indiana à Paris en 1832.".into(),
            page_title: Some("George Sand".into()),
            subject_label: Some("George Sand".into()),
            document_type: "article".into(),
            subject_death_year: Some(1876),
            ..Default::default()
        });
        assert_eq!(travel.len(), 1);
        assert_eq!(travel[0].event_type, "residence");
        assert_eq!(travel[0].time_surface.as_deref(), Some("1831"));
    }
}
