// crates/talaria-sources/src/extractors/itinerary.rs
//! Itinerary steps: departure / arrival / passage — never interpolated.

use crate::extractors::travel::find_year;
use crate::extractors::{CandidateExtractor, ExtractorInput, RawCandidate};
use crate::place_quality::is_plausible_place_label;

pub struct ItineraryExtractor;

impl CandidateExtractor for ItineraryExtractor {
    fn extractor_id(&self) -> &str {
        "itinerary"
    }

    fn version(&self) -> &str {
        "itinerary:v1"
    }

    fn extract(&self, input: &ExtractorInput) -> Vec<RawCandidate> {
        let subject = input.effective_subject();
        let mut out = Vec::new();
        for (i, line) in input.text.lines().enumerate() {
            let lower = line.to_lowercase();
            let (etype, pred, seq) = if lower.contains("departed")
                || lower.contains("left for")
                || lower.contains("set out for")
                || lower.contains("sailed for")
                || lower.contains("embarked")
            {
                ("departure", "departed_for", Some("depart"))
            } else if lower.contains("arrived at")
                || lower.contains("arrived in")
                || lower.contains("entered ")
                || lower.contains("reached ")
                || lower.contains("landed at")
                || lower.contains("disembarked")
            {
                ("arrival", "arrived_in", Some("arrive"))
            } else if lower.contains("passed through")
                || lower.contains("stopped at")
                || lower.contains("via ")
            {
                ("passage", "passed_through", Some("pass"))
            } else if lower.contains("returned to") {
                ("arrival", "returned_to", Some("return"))
            } else {
                continue;
            };
            let place = extract_place(line).filter(|p| is_plausible_place_label(p));
            let year = find_year(line);
            // Require place + year to avoid noise
            let Some(place) = place else { continue };
            let Some(year) = year else { continue };
            out.push(RawCandidate {
                event_type: etype.into(),
                predicate: pred.into(),
                subject_surface: subject.clone(),
                time_surface: Some(year),
                place_surface: Some(place),
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
            let _ = seq;
        }
        out
    }
}

fn extract_place(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    // Prefer cues tied to motion verbs (left→right scan), not trailing "in March".
    for cue in [
        "departed for ",
        "left for ",
        "set out for ",
        "sailed for ",
        "arrived at ",
        "arrived in ",
        "landed at ",
        "passed through ",
        "stopped at ",
        "returned to ",
        "reached ",
        "entered ",
        "embarked for ",
        "disembarked at ",
    ] {
        if let Some(pos) = lower.find(cue) {
            let after = &line[pos + cue.len()..];
            let token = after
                .split(|c: char| c == '.' || c == ',' || c == ';' || c.is_ascii_digit())
                .next()?
                .trim();
            // Drop trailing prepositional tails like "in March"
            let token = token
                .split(" in ")
                .next()
                .unwrap_or(token)
                .trim()
                .trim_matches(|c: char| !c.is_alphabetic() && c != ' ' && c != '-' && c != '\'')
                .to_string();
            if token.len() >= 2 {
                return Some(token);
            }
        }
    }
    for cue in [" for ", " to ", " at ", " through ", " via "] {
        if let Some(pos) = lower.find(cue) {
            let after = &line[pos + cue.len()..];
            let token = after
                .split(|c: char| c == '.' || c == ',' || c == ';' || c.is_ascii_digit())
                .next()?
                .trim()
                .split(" in ")
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if token.len() >= 2 {
                return Some(token);
            }
        }
    }
    None
}
