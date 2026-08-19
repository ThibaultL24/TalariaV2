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
        for (i, line) in input.text.lines().enumerate() {
            let lower = line.to_lowercase();
            let (etype, pred) = if lower.contains("departed") || lower.contains("left for") {
                ("departure", "departed_for")
            } else if lower.contains("arrived") {
                ("arrival", "arrived_in")
            } else if lower.contains("lived in") || lower.contains("resided") {
                ("residence", "resided_in")
            } else if lower.contains("stayed in") || lower.contains("stayed at") {
                ("residence", "stayed_at")
            } else if lower.contains("exiled") {
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
    for cue in [" in ", " at ", " to ", " for "] {
        if let Some(pos) = lower.rfind(cue) {
            let after = &s[pos + cue.len()..];
            let token = after
                .split(|c: char| c == '.' || c.is_ascii_digit() || c == ',')
                .next()?
                .trim()
                .trim_matches(|c: char| !c.is_alphabetic() && c != ' ' && c != '-')
                .to_string();
            if token.len() >= 2 {
                return Some(token);
            }
        }
    }
    None
}
