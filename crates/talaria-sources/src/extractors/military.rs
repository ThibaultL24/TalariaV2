// crates/talaria-sources/src/extractors/military.rs
//! Military campaign / battle page extractor — one page can yield multiple steps.

use crate::extractors::{CandidateExtractor, ExtractorInput, RawCandidate};
use crate::place_quality::is_plausible_place_label;

pub struct MilitaryCampaignExtractor;

impl CandidateExtractor for MilitaryCampaignExtractor {
    fn extractor_id(&self) -> &str {
        "military_campaign"
    }

    fn version(&self) -> &str {
        "military_campaign:v1"
    }

    fn extract(&self, input: &ExtractorInput) -> Vec<RawCandidate> {
        let mut out = Vec::new();
        let subject = input.effective_subject();
        let title = input.page_title.clone().unwrap_or_default();

        // Page-level battle/siege → at least one occurrence (place from title).
        if let Some((etype, place, object)) = classify_page_title(&title) {
            let year = first_year(&input.text);
            let place_opt = if place.is_empty() { None } else { Some(place) };
            out.push(RawCandidate {
                event_type: etype.into(),
                predicate: if etype == "battle" {
                    "fought_at"
                } else if etype == "siege" {
                    "besieged"
                } else if etype == "treaty" {
                    "signed"
                } else {
                    "campaign_at"
                }
                .into(),
                subject_surface: subject.clone(),
                time_surface: year,
                place_surface: place_opt,
                object_surface: Some(object),
                participant_surfaces: vec![],
                clause_text: title.clone(),
                clause_index: 0,
                start_offset: 0,
                end_offset: title.len() as i32,
                cross_clause_join: false,
                extractor_id: self.extractor_id().into(),
                is_posthumous: false,
            });
        }

        // Line-level military cues
        for (i, line) in input.text.lines().enumerate() {
            let lower = line.to_lowercase();
            let (etype, pred) = if lower.contains("siege of") || lower.contains("besieged") {
                ("siege", "besieged")
            } else if lower.contains("battle of")
                || lower.contains("fought at")
                || lower.contains("defeated")
                || lower.contains("victory at")
            {
                ("battle", "fought_at")
            } else if lower.contains("retreated") || lower.contains("retreat from") {
                ("retreat", "retreated_from")
            } else if lower.contains("surrender") || lower.contains("capitulat") {
                ("surrender", "surrendered_at")
            } else if lower.contains("headquarters") || lower.contains("quartier général") {
                ("headquarters", "hq_at")
            } else if lower.contains("campaign")
                && (lower.contains("began") || lower.contains("opened"))
            {
                ("military_campaign", "campaign_at")
            } else {
                continue;
            };
            let year = first_year(line);
            let place = place_from_battle_phrase(line)
                .or_else(|| place_from_line(line))
                .filter(|p| is_plausible_place_label(p));
            // Line-level cues need a year + plausible place to avoid prose noise.
            let Some(year) = year else { continue };
            let Some(place) = place else { continue };
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
            });
        }
        out
    }
}

fn classify_page_title(title: &str) -> Option<(&'static str, String, String)> {
    let t = title.trim();
    let lower = t.to_lowercase();
    if let Some(rest) = lower.strip_prefix("battle of ") {
        let place = title_case_place(&t[t.len() - rest.len()..]);
        return Some(("battle", place.clone(), t.to_string()));
    }
    if let Some(rest) = lower.strip_prefix("siege of ") {
        let place = title_case_place(&t[t.len() - rest.len()..]);
        return Some(("siege", place.clone(), t.to_string()));
    }
    if lower.contains("campaign") {
        // Campaign pages are multi-step containers — no single place from the title.
        return Some(("military_campaign", String::new(), t.to_string()));
    }
    if lower.starts_with("treaty of ") || lower.starts_with("treaties of ") {
        let place = t
            .split_once(" of ")
            .map(|(_, p)| p.to_string())
            .unwrap_or_else(|| t.to_string());
        return Some(("treaty", place, t.to_string()));
    }
    None
}

fn title_case_place(s: &str) -> String {
    s.split('(')
        .next()
        .unwrap_or(s)
        .trim()
        .trim_end_matches(['.', ','])
        .to_string()
}

fn first_year(text: &str) -> Option<String> {
    for w in text.split(|c: char| !c.is_ascii_digit()) {
        if w.len() == 4 {
            if let Ok(y) = w.parse::<i32>() {
                if (1700..=1900).contains(&y) {
                    return Some(y.to_string());
                }
            }
        }
    }
    None
}

fn place_from_battle_phrase(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    for prefix in ["battle of ", "siege of "] {
        if let Some(pos) = lower.find(prefix) {
            let after = &line[pos + prefix.len()..];
            let token = after
                .split(|c: char| c == '.' || c == ',' || c == ';' || c.is_ascii_digit())
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

fn place_from_line(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    for cue in [" near ", " at ", " in "] {
        if let Some(pos) = lower.find(cue) {
            let after = &line[pos + cue.len()..];
            let token = after
                .split(|c: char| c == '.' || c == ',' || c.is_ascii_digit())
                .next()?
                .trim()
                .to_string();
            if token.len() >= 2 && token.chars().next()?.is_uppercase() {
                return Some(token);
            }
        }
    }
    None
}
