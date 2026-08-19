// crates/talaria-quality/src/analyzer.rs
//! ClauseAnalyzer trait + deterministic test adapter.
//! COSMOS interface is declared; no fake COSMOS adapter ships in Livrable 1.

use crate::model::EXTRACTOR_DETERMINISTIC_V1;
use serde::{Deserialize, Serialize};

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

pub const COSMOS_HEURISTIC_ID: &str = "cosmos-heuristic";
pub const COSMOS_HEURISTIC_V1: &str = "heuristic:v1";
pub const COSMOS_DEFAULT_MIN_SCORE: f32 = 0.45;

/// Observed tuple — never invented. Incomplete extractions stay out of this list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CosmosTuple {
    pub person: String,
    pub time: String,
    pub place: String,
    #[serde(default)]
    pub verb: Option<String>,
}

/// Fragment filter verdict. `accepted` is not a historical fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CosmosJudgment {
    pub score: f32,
    pub accepted: bool,
    pub signals: Vec<String>,
    pub tuples: Vec<CosmosTuple>,
    pub reject_reason: Option<String>,
    pub analyzer_id: String,
    pub version: String,
}

/// COSMOS as an auditable filter in front of extractors.
pub trait CosmosClauseAnalyzer: ClauseAnalyzer {
    fn cosmos_model_id(&self) -> &str;
    fn judge_fragment(&self, input: &ClauseAnalyzeInput) -> CosmosJudgment;
}

/// Deterministic cheap+score filter. No Napoleonic year window. No invented tuples.
pub struct HeuristicCosmosAnalyzer {
    pub min_score: f32,
}

impl Default for HeuristicCosmosAnalyzer {
    fn default() -> Self {
        Self {
            min_score: COSMOS_DEFAULT_MIN_SCORE,
        }
    }
}

impl HeuristicCosmosAnalyzer {
    pub fn new(min_score: f32) -> Self {
        Self {
            min_score: min_score.clamp(0.0, 1.0),
        }
    }
}

impl ClauseAnalyzer for HeuristicCosmosAnalyzer {
    fn analyzer_id(&self) -> &str {
        COSMOS_HEURISTIC_ID
    }

    fn version(&self) -> &str {
        COSMOS_HEURISTIC_V1
    }

    fn analyze_sentence(&self, _input: &ClauseAnalyzeInput) -> Vec<ClauseExtraction> {
        Vec::new()
    }
}

impl CosmosClauseAnalyzer for HeuristicCosmosAnalyzer {
    fn cosmos_model_id(&self) -> &str {
        COSMOS_HEURISTIC_V1
    }

    fn judge_fragment(&self, input: &ClauseAnalyzeInput) -> CosmosJudgment {
        judge_heuristic(input, self.min_score, COSMOS_HEURISTIC_ID, COSMOS_HEURISTIC_V1)
    }
}

fn judge_heuristic(
    input: &ClauseAnalyzeInput,
    min_score: f32,
    analyzer_id: &str,
    version: &str,
) -> CosmosJudgment {
    let text = input.text.trim();
    if text.is_empty() || !text.chars().any(|c| c.is_alphabetic()) {
        return CosmosJudgment {
            score: 0.0,
            accepted: false,
            signals: vec![],
            tuples: vec![],
            reject_reason: Some("empty".into()),
            analyzer_id: analyzer_id.into(),
            version: version.into(),
        };
    }

    let year = crate::time_typed::extract_time_surface(text).and_then(|s| {
        crate::time_typed::typed_time_year(&crate::time_typed::parse_typed_time(Some(&s)))
            .map(|y| y.to_string())
    });
    let verb = heuristic_verb(text);
    let place = crate::resolve::gazetteer_place_in_text(text)
        .or_else(|| extract_place_after_cue(text, &text.to_lowercase()));

    let person = person_surface_in_text(text);
    let mut signals = Vec::new();
    let mut score = 0.0_f32;
    if year.is_some() {
        score += 0.30;
        signals.push("year".into());
    }
    if verb.is_some() {
        score += 0.30;
        signals.push("verb_cue".into());
    }
    if place.is_some() {
        score += 0.25;
        signals.push("place_hit".into());
    }

    let mut tuples = Vec::new();
    if let (Some(person), Some(time), Some(place)) = (person, year.clone(), place.clone()) {
        score += 0.15;
        signals.push("cosmos_tuple".into());
        tuples.push(CosmosTuple {
            person,
            time,
            place,
            verb: verb.clone(),
        });
    }
    let score = score.min(1.0);
    let accepted = score >= min_score;
    CosmosJudgment {
        score,
        accepted,
        signals,
        tuples,
        reject_reason: if accepted {
            None
        } else if score == 0.0 {
            Some("no_signals".into())
        } else {
            Some("below_threshold".into())
        },
        analyzer_id: analyzer_id.into(),
        version: version.into(),
    }
}

fn person_surface_in_text(clause: &str) -> Option<String> {
    let name = subject_from_title_or_clause(None, clause);
    if name == "Unknown" {
        None
    } else {
        Some(name)
    }
}

fn heuristic_verb(clause: &str) -> Option<String> {
    if let Some((_, pred)) = classify_predicate(clause) {
        return Some(pred.to_string());
    }
    let lower = clause.to_lowercase();
    const EXTRA: &[(&str, &str)] = &[
        ("discovered", "discovered"),
        ("published", "published"),
        ("studied", "studied"),
        ("visited", "visited"),
        ("lived", "lived"),
        ("resided", "lived"),
    ];
    for (cue, verb) in EXTRA {
        if lower.contains(cue) {
            return Some((*verb).to_string());
        }
    }
    None
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

fn extract_place_after_cue(clause: &str, lower: &str) -> Option<String> {
    // Prefer last geographic cue; stop before years and further cues.
    let mut best = None;
    for cue in [" in ", " at ", " to ", " on ", " near "] {
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
            let time_surface = crate::time_typed::extract_time_surface(&clause_text);
            let (mut place_surface, object_surface) = find_place_or_person_object(&clause_text);
            if place_surface.is_none() {
                place_surface = crate::resolve::gazetteer_place_in_text(&clause_text);
            }

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
        let input = ClauseAnalyzeInput {
            text: "In 1774 his father died; he fought in Leipzig.".into(),
            page_title: Some("Napoleon".into()),
            start_offset: 0,
        };
        let xs = analyzer.analyze_sentence(&input);
        for x in &xs {
            if x.event_type == "battle" {
                assert_ne!(x.time_surface.as_deref(), Some("1774"));
                assert_eq!(x.place_surface.as_deref(), Some("Leipzig"));
            }
        }
    }

    #[test]
    fn heuristic_keeps_relevant_phrase() {
        let analyzer = HeuristicCosmosAnalyzer::default();
        let j = analyzer.judge_fragment(&ClauseAnalyzeInput {
            text: "Napoleon Bonaparte was born on 15 August 1769 in Ajaccio.".into(),
            page_title: Some("Napoleon".into()),
            start_offset: 0,
        });
        assert!(j.accepted, "{j:?}");
        assert!(j.signals.iter().any(|s| s == "year"));
        assert!(j.signals.iter().any(|s| s == "verb_cue"));
        assert!(j.score >= COSMOS_DEFAULT_MIN_SCORE);
    }

    #[test]
    fn heuristic_rejects_empty_phrase() {
        let analyzer = HeuristicCosmosAnalyzer::default();
        let j = analyzer.judge_fragment(&ClauseAnalyzeInput {
            text: "   ".into(),
            page_title: None,
            start_offset: 0,
        });
        assert!(!j.accepted);
        assert_eq!(j.score, 0.0);
        assert_eq!(j.reject_reason.as_deref(), Some("empty"));
        assert!(j.tuples.is_empty());
    }

    #[test]
    fn heuristic_has_no_napoleonic_year_window() {
        let analyzer = HeuristicCosmosAnalyzer::default();
        let j = analyzer.judge_fragment(&ClauseAnalyzeInput {
            text: "Marie Curie discovered radium in Paris in 1898.".into(),
            page_title: Some("Marie Curie".into()),
            start_offset: 0,
        });
        assert!(j.accepted, "1898 must not be dropped: {j:?}");
        assert!(j.signals.iter().any(|s| s == "year"));
    }

    #[test]
    fn heuristic_does_not_extract_events() {
        let analyzer = HeuristicCosmosAnalyzer::default();
        let xs = analyzer.analyze_sentence(&ClauseAnalyzeInput {
            text: "Napoleon Bonaparte was born on 15 August 1769 in Ajaccio.".into(),
            page_title: Some("Napoleon".into()),
            start_offset: 0,
        });
        assert!(xs.is_empty());
    }
}
