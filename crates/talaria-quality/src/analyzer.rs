// crates/talaria-quality/src/analyzer.rs
//! ClauseAnalyzer trait + deterministic test adapter.
//! COSMOS interface is declared; no fake COSMOS adapter ships in Livrable 1.

use crate::model::EXTRACTOR_DETERMINISTIC_V1;

#[derive(Debug, Clone)]
pub struct ClauseAnalyzeInput {
    pub text: String,
    pub page_title: Option<String>,
    /// Absolute start offset of this sentence within the snapshot text.
    pub start_offset: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClauseExtraction {
    pub clause_index: i32,
    pub clause_text: String,
    pub clause_start_offset: i32,
    pub clause_end_offset: i32,
    pub subject_surface: String,
    pub event_type: String,
    pub predicate: String,
    pub time_surface: Option<String>,
    pub place_surface: Option<String>,
    pub object_surface: Option<String>,
    pub participant_surfaces: Vec<String>,
    /// Set when extraction would have required signals from another clause.
    pub cross_clause_join: bool,
}

pub trait ClauseAnalyzer: Send + Sync {
    fn analyzer_id(&self) -> &str;
    fn version(&self) -> &str;
    fn analyze_sentence(&self, input: &ClauseAnalyzeInput) -> Vec<ClauseExtraction>;
}

/// Clear extension point for a future real COSMOS implementation.
/// Livrable 1 does NOT provide a fake COSMOS adapter.
#[allow(dead_code)]
pub trait CosmosClauseAnalyzer: ClauseAnalyzer {
    fn cosmos_model_id(&self) -> &str;
}

/// Split sentence into clauses on strong delimiters only.
pub fn split_clauses(sentence: &str) -> Vec<(i32, String, i32, i32)> {
    let mut clauses = Vec::new();
    let mut start = 0usize;
    let bytes = sentence.as_bytes();
    let mut idx = 0usize;
    let mut clause_index = 0i32;

    while idx < bytes.len() {
        let is_delim = matches!(bytes[idx], b';' | b'.');
        if is_delim || idx + 1 == bytes.len() {
            let end = if is_delim { idx } else { idx + 1 };
            let slice = sentence[start..end].trim();
            if !slice.is_empty() {
                let abs_start = start as i32;
                let abs_end = end as i32;
                clauses.push((clause_index, slice.to_string(), abs_start, abs_end));
                clause_index += 1;
            }
            start = idx + 1;
        }
        idx += 1;
    }
    if clauses.is_empty() && !sentence.trim().is_empty() {
        let t = sentence.trim();
        clauses.push((0, t.to_string(), 0, t.len() as i32));
    }
    clauses
}

fn classify_predicate(clause: &str) -> Option<(&'static str, &'static str)> {
    let lower = clause.to_lowercase();
    const RULES: &[(&[&str], &str, &str)] = &[
        (&["was born", "born in", "born at"], "birth", "born_in"),
        (&["died", "death of", "passed away"], "death", "died_in"),
        (&["married", "wedding"], "marriage", "married"),
        (&["divorced"], "divorce", "divorced"),
        (
            &["fought", "battle of", "defeated", "victory at"],
            "battle",
            "fought_at",
        ),
        (&["exiled", "exile to"], "exile", "exiled_to"),
        (&["crowned", "abdicated"], "office", "held_office"),
        (&["signed", "treaty"], "diplomatic", "signed"),
        (&["met with", "meeting"], "meeting", "met"),
    ];
    for (cues, et, pred) in RULES {
        if cues.iter().any(|c| lower.contains(c)) {
            return Some((*et, *pred));
        }
    }
    None
}

fn find_year(clause: &str) -> Option<String> {
    let mut years = Vec::new();
    for word in clause.split(|c: char| !c.is_ascii_digit()) {
        if word.len() == 4 {
            if let Ok(y) = word.parse::<i32>() {
                if (1000..=2100).contains(&y) {
                    years.push(y.to_string());
                }
            }
        }
    }
    // Same-clause only: take first year in this clause.
    years.into_iter().next()
}

fn extract_place_after_cue(clause: &str, lower: &str) -> Option<String> {
    // Prefer last geographic cue; stop before years and further cues.
    let mut best = None;
    for cue in [" in ", " at ", " to "] {
        let mut search_from = 0usize;
        while let Some(rel) = lower[search_from..].find(cue) {
            let pos = search_from + rel;
            let after = &clause[pos + cue.len()..];
            let token = after
                .split(|c: char| c == '.' || c == ';' || c == ',' || c.is_ascii_digit())
                .next()
                .unwrap_or(after);
            let token = token
                .split(" in ")
                .next()
                .unwrap_or(token)
                .split(" at ")
                .next()
                .unwrap_or(token)
                .split(" to ")
                .next()
                .unwrap_or(token)
                .trim()
                .trim_matches(|c: char| !c.is_alphabetic() && c != ' ' && c != '-' && c != '\'')
                .to_string();
            if token.len() >= 2 {
                best = Some(token);
            }
            search_from = pos + cue.len();
        }
    }
    best
}

fn find_place_or_person_object(clause: &str) -> (Option<String>, Option<String>) {
    let lower = clause.to_lowercase();
    let mut object = None;
    if let Some(pos) = lower.find(" married ") {
        let after = clause[pos + " married ".len()..].trim();
        let name = after
            .split(" in ")
            .next()
            .unwrap_or(after)
            .split(" at ")
            .next()
            .unwrap_or(after)
            .split(|c: char| c == '.' || c == ';' || c == ',')
            .next()
            .unwrap_or(after)
            .trim()
            .trim_matches(|c: char| {
                !c.is_alphabetic() && c != ' ' && c != '-' && c != '\'' && c != 'é' && c != 'è'
            })
            .to_string();
        if !name.is_empty() {
            object = Some(name);
        }
    }
    let place = extract_place_after_cue(clause, &lower);
    // If object equals place token (no spouse), keep as place only.
    if object.as_ref() == place.as_ref() {
        return (place, None);
    }
    (place, object)
}

fn subject_from_title_or_clause(page_title: Option<&str>, clause: &str) -> String {
    if let Some(title) = page_title {
        let base = title.split('(').next().unwrap_or(title).trim();
        if !base.is_empty() && clause.to_lowercase().contains(&base.to_lowercase())
            || clause.to_lowercase().contains("he ")
            || clause.to_lowercase().contains("she ")
            || clause.starts_with("He ")
            || clause.starts_with("She ")
        {
            return base.to_string();
        }
        if !base.is_empty() {
            return base.to_string();
        }
    }
    // Fallback: first Capitalized token sequence
    let mut parts = Vec::new();
    for w in clause.split_whitespace() {
        let clean = w.trim_matches(|c: char| !c.is_alphabetic() && c != '-');
        if clean.chars().next().is_some_and(|c| c.is_uppercase()) {
            parts.push(clean.to_string());
        } else if !parts.is_empty() {
            break;
        }
    }
    if parts.is_empty() {
        "Unknown".into()
    } else {
        parts.join(" ")
    }
}

/// Deterministic, same-clause-only extractor for tests and offline runs.
pub struct DeterministicClauseAnalyzer;

impl ClauseAnalyzer for DeterministicClauseAnalyzer {
    fn analyzer_id(&self) -> &str {
        "deterministic"
    }

    fn version(&self) -> &str {
        EXTRACTOR_DETERMINISTIC_V1
    }

    fn analyze_sentence(&self, input: &ClauseAnalyzeInput) -> Vec<ClauseExtraction> {
        let mut out = Vec::new();
        let clauses = split_clauses(&input.text);

        for (clause_index, clause_text, rel_start, rel_end) in clauses {
            let Some((event_type, predicate)) = classify_predicate(&clause_text) else {
                continue;
            };
            let time_surface = find_year(&clause_text);
            let (place_surface, object_surface) = find_place_or_person_object(&clause_text);

            // Dense extraction restricted to same clause: we never pull year/place
            // from sibling clauses. Flag would only be set by a buggy join path.
            let cross_clause_join = false;

            let abs_start = input.start_offset + rel_start;
            let abs_end = input.start_offset + rel_end;

            out.push(ClauseExtraction {
                clause_index,
                clause_text: clause_text.clone(),
                clause_start_offset: abs_start,
                clause_end_offset: abs_end,
                subject_surface: subject_from_title_or_clause(
                    input.page_title.as_deref(),
                    &clause_text,
                ),
                event_type: event_type.into(),
                predicate: predicate.into(),
                time_surface,
                place_surface,
                object_surface,
                participant_surfaces: vec![],
                cross_clause_join,
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_cross_clause_year_place_join() {
        let analyzer = DeterministicClauseAnalyzer;
        let input = ClauseAnalyzeInput {
            text: "He was born in Ajaccio. He later fought at Leipzig in 1813.".into(),
            page_title: Some("Napoleon".into()),
            start_offset: 0,
        };
        // With semicolon/period split, birth clause has no 1813; battle has Leipzig+1813.
        let xs = analyzer.analyze_sentence(&input);
        // "He was born in Ajaccio" — birth, place Ajaccio, no year
        // "He later fought at Leipzig in 1813" — battle, Leipzig, 1813
        assert!(xs.iter().all(|x| !x.cross_clause_join));
        let birth = xs.iter().find(|x| x.event_type == "birth");
        let battle = xs.iter().find(|x| x.event_type == "battle");
        assert!(birth.is_some());
        assert!(battle.is_some());
        // Must NOT invent birth@Leipzig/1813 or battle without its own year from other clause
        let birth = birth.unwrap();
        assert_eq!(birth.place_surface.as_deref(), Some("Ajaccio"));
        assert!(birth.time_surface.is_none());
        let battle = battle.unwrap();
        assert_eq!(battle.place_surface.as_deref(), Some("Leipzig"));
        assert_eq!(battle.time_surface.as_deref(), Some("1813"));
    }

    #[test]
    fn rejects_joining_1774_from_other_clause() {
        let analyzer = DeterministicClauseAnalyzer;
        // Classic noise pattern: year in clause A, place+verb in clause B
        let input = ClauseAnalyzeInput {
            text: "In 1774 his father died; he fought in Leipzig.".into(),
            page_title: Some("Napoleon".into()),
            start_offset: 0,
        };
        let xs = analyzer.analyze_sentence(&input);
        // Battle clause must not absorb 1774 from the previous clause.
        for x in &xs {
            if x.event_type == "battle" {
                assert_ne!(x.time_surface.as_deref(), Some("1774"));
                assert_eq!(x.place_surface.as_deref(), Some("Leipzig"));
            }
        }
    }
}
