// crates/talaria-sources/src/extractors/timeline.rs
use crate::extractors::{CandidateExtractor, ExtractorInput, RawCandidate};

/// Chronology list / bullet extractor: `* YYYY — Place — description`
pub struct TimelineListExtractor;

impl CandidateExtractor for TimelineListExtractor {
    fn extractor_id(&self) -> &str {
        "timeline_list"
    }

    fn version(&self) -> &str {
        "timeline_list:v1"
    }

    fn extract(&self, input: &ExtractorInput) -> Vec<RawCandidate> {
        let subject = input.effective_subject();
        let mut out = Vec::new();
        for (i, line) in input.text.lines().enumerate() {
            let raw = line.trim().trim_start_matches(['*', '-', '•']).trim();
            // 1793 — Toulon — siege
            let parts: Vec<&str> = raw.split("—").map(str::trim).collect();
            if parts.len() < 2 {
                continue;
            }
            let year = parts[0];
            if year.len() != 4 || !year.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let year_i: i32 = year.parse().unwrap_or(0);
            let place = if parts.len() >= 2 {
                Some(parts[1].to_string())
            } else {
                None
            };
            let desc = parts.get(2).copied().unwrap_or("").to_lowercase();
            let (event_type, predicate) = classify_desc(&desc);
            let is_posthumous = input
                .subject_death_year
                .is_some_and(|d| year_i > d);
            let et = if is_posthumous && matches!(event_type, "memorial" | "commemoration" | "life_event") {
                "commemoration".to_string()
            } else if is_posthumous {
                "commemoration".to_string()
            } else {
                event_type.to_string()
            };
            let start = 0i32;
            out.push(RawCandidate {
                event_type: et,
                predicate: predicate.into(),
                subject_surface: subject.clone(),
                time_surface: Some(year.to_string()),
                place_surface: place,
                object_surface: None,
                participant_surfaces: vec![],
                clause_text: raw.to_string(),
                clause_index: i as i32,
                start_offset: start,
                end_offset: start + raw.len() as i32,
                cross_clause_join: false,
                extractor_id: self.extractor_id().into(),
                is_posthumous,
                lat: None,
                lon: None,
            });
        }
        out
    }
}

fn classify_desc(desc: &str) -> (&'static str, &'static str) {
    if desc.contains("battle") || desc.contains("siege") {
        ("battle", "fought_at")
    } else if desc.contains("treaty") || desc.contains("peace") {
        ("diplomatic", "signed")
    } else if desc.contains("coronation") || desc.contains("crowned") {
        ("office", "held_office")
    } else if desc.contains("campaign") || desc.contains("retreat") || desc.contains("invade") {
        ("military_campaign", "campaign_at")
    } else if desc.contains("remain") || desc.contains("posthumous") || desc.contains("returned") {
        ("commemoration", "commemorated_at")
    } else {
        ("life_event", "occurred_at")
    }
}
