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
            out.push(RawCandidate {
                event_type: "commemoration".into(),
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
            });
        }
        out
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
