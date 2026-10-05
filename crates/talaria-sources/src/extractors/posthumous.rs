// crates/talaria-sources/src/extractors/posthumous.rs
use crate::extractors::travel::find_year;
use crate::extractors::{CandidateExtractor, ExtractorInput, RawCandidate};

/// Marks commemorations after subject death — never as active deeds of the subject.
pub struct PosthumousEventExtractor;

impl CandidateExtractor for PosthumousEventExtractor {
    fn extractor_id(&self) -> &str {
        "posthumous"
    }

    fn version(&self) -> &str {
        "posthumous:v1"
    }

    fn extract(&self, input: &ExtractorInput) -> Vec<RawCandidate> {
        let Some(death) = input.subject_death_year else {
            return vec![];
        };
        let subject = input.effective_subject();
        let mut out = Vec::new();
        for (i, line) in input.text.lines().enumerate() {
            let lower = line.to_lowercase();
            let commemorative = lower.contains("statue")
                || lower.contains("museum")
                || lower.contains("remains")
                || lower.contains("commemorat")
                || lower.contains("memorial")
                || lower.contains("reburied")
                || lower.contains("returned");
            if !commemorative {
                continue;
            }
            let Some(year_s) = find_year(line) else {
                continue;
            };
            let Ok(year) = year_s.parse::<i32>() else {
                continue;
            };
            if year <= death {
                continue;
            }
            let event_type = classify_commemorative_type(&lower);
            out.push(RawCandidate {
                event_type,
                predicate: "commemorated_at".into(),
                subject_surface: subject.clone(),
                time_surface: Some(year_s),
                place_surface: find_place_simple(line),
                object_surface: None,
                participant_surfaces: vec![],
                clause_text: line.trim().to_string(),
                clause_index: i as i32,
                start_offset: 0,
                end_offset: line.len() as i32,
                cross_clause_join: false,
                extractor_id: self.extractor_id().into(),
                is_posthumous: true,
                lat: None,
                lon: None,
            });
        }
        out
    }
}

fn classify_commemorative_type(lower: &str) -> String {
    if lower.contains("museum") || lower.contains("musée") {
        "museum".into()
    } else if lower.contains("statue") || lower.contains("monument") || lower.contains("column") {
        "statue".into()
    } else if lower.contains("memorial") || lower.contains("plaque") || lower.contains("tomb") {
        "memorial".into()
    } else if lower.contains("street") || lower.contains("avenue") || lower.contains("square") {
        "street_naming".into()
    } else {
        "memorial".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_museum_and_statue_keywords() {
        assert_eq!(classify_commemorative_type("a new museum opened"), "museum");
        assert_eq!(classify_commemorative_type("statue unveiled"), "statue");
    }
}

fn find_place_simple(s: &str) -> Option<String> {
    let lower = s.to_lowercase();
    for cue in [" in ", " at ", " to "] {
        if let Some(pos) = lower.rfind(cue) {
            let after = &s[pos + cue.len()..];
            let token = after
                .split(|c: char| c == '.' || c.is_ascii_digit() || c == ',')
                .next()?
                .trim()
                .to_string();
            if token.len() >= 2 {
                return Some(token);
            }
        }
    }
    None
}
