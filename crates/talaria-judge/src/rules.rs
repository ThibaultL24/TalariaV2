// crates/talaria-judge/src/rules.rs
//! Rule-based judge: category (event_type) × epistemic_status.
//! COSMOS tuples are ClaimCandidates — never silent authority.

use crate::place::parse_place_surface;
use crate::time::parse_time_surface;
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct CandidateInput {
    pub person_surface: String,
    pub time_surface: Option<String>,
    pub place_surface: Option<String>,
    pub verb_pivot: Option<String>,
    pub sentence_text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JudgeLabel {
    Accept,
    Reject,
}

#[derive(Debug, Clone)]
pub struct JudgeVerdict {
    pub label: JudgeLabel,
    pub score: f64,
    pub reason: String,
    pub event_type: String,
    pub epistemic_status: String,
    pub title: String,
    pub summary: String,
    pub start_time: Option<DateTime<Utc>>,
    pub time_json: Value,
    pub place_label: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub map_eligible: bool,
    pub confidence: f64,
}

struct Classified {
    event_type: String,
    verb_label: String,
    family: &'static str,
}

pub fn judge_candidate(input: &CandidateInput) -> JudgeVerdict {
    let mut score: f64 = 0.35;
    let mut reasons = Vec::new();

    let time_surface = match input.time_surface.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(value) => value,
        None => return reject(input, 0.1, "missing time_surface"),
    };

    let place_surface = match input.place_surface.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(value) => value,
        None => return reject(input, 0.1, "missing place_surface"),
    };

    let parsed_time = match parse_time_surface(time_surface) {
        Some(value) => {
            score += 0.25;
            reasons.push("time_parsed");
            value
        }
        None => return reject(input, 0.2, "unparseable time_surface"),
    };

    let parsed_place = parse_place_surface(place_surface);
    score += 0.15;
    reasons.push("place_present");
    if parsed_place.map_eligible() {
        score += 0.1;
        reasons.push("place_geocoded");
    }

    let verb = input
        .verb_pivot
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    let sentence_lc = input.sentence_text.to_lowercase();

    let classified = classify(&verb, &sentence_lc);
    if classified.family != "other" {
        score += 0.15;
        reasons.push("category_classified");
    } else if !verb.is_empty() {
        score += 0.08;
        reasons.push("life_event_fallback");
    }

    if input.person_surface.trim().len() >= 3 {
        score += 0.05;
    }

    let epistemic = infer_epistemic_status(&sentence_lc, classified.family);
    reasons.push(match epistemic.as_str() {
        "rumor" => "epistemic_rumor_cue",
        "theory" => "epistemic_theory_cue",
        "uncertain" => "epistemic_uncertain",
        "established" => "epistemic_strong_pattern",
        _ => "epistemic_attested_source",
    });

    match epistemic.as_str() {
        "rumor" | "theory" => score = (score - 0.05).max(0.55),
        "uncertain" => score = (score - 0.08).max(0.55),
        "established" => score = (score + 0.05).min(1.0),
        _ => {}
    }

    let confidence = score.min(1.0);
    // Show everything with person+time+place: accept from 0.55 (minor / rumor included).
    if confidence < 0.55 {
        return reject(input, confidence, "score below threshold");
    }

    let title = format!(
        "{} {} in {} ({})",
        input.person_surface.trim(),
        classified.verb_label,
        parsed_place.label,
        parsed_time.surface
    );

    JudgeVerdict {
        label: JudgeLabel::Accept,
        score: confidence,
        reason: reasons.join(","),
        event_type: classified.event_type,
        epistemic_status: epistemic,
        title,
        summary: input.sentence_text.trim().to_string(),
        start_time: Some(parsed_time.start),
        time_json: parsed_time.to_json(),
        place_label: Some(parsed_place.label.clone()),
        lat: parsed_place.lat,
        lon: parsed_place.lon,
        map_eligible: parsed_place.map_eligible(),
        confidence,
    }
}

fn classify(verb: &str, sentence_lc: &str) -> Classified {
    if let Some(c) = classify_commemorative(sentence_lc) {
        return c;
    }
    if let Some(c) = classify_verb(verb) {
        return c;
    }
    if let Some(c) = classify_sentence_cues(sentence_lc) {
        return c;
    }
    Classified {
        event_type: "life_event".into(),
        verb_label: if verb.is_empty() {
            "associated".into()
        } else {
            verb.to_string()
        },
        family: "other",
    }
}

fn classify_commemorative(sentence_lc: &str) -> Option<Classified> {
    let cues: &[(&[&str], &str, &str)] = &[
        (&["statue", "monument", "bust of"], "statue", "statue unveiled"),
        (&["museum"], "museum", "museum opened"),
        (
            &[
                "street named",
                "avenue named",
                "boulevard named",
                "square named",
                "renamed",
                "named after",
            ],
            "street_naming",
            "named",
        ),
        (&["memorial"], "memorial", "memorial dedicated"),
        (&["plaque"], "memorial", "plaque unveiled"),
    ];
    for (needles, event_type, label) in cues {
        if needles.iter().any(|n| sentence_lc.contains(n)) {
            return Some(Classified {
                event_type: (*event_type).into(),
                verb_label: (*label).into(),
                family: "legacy",
            });
        }
    }
    None
}

fn classify_verb(verb: &str) -> Option<Classified> {
    let rules: &[(&[&str], &str, &str, &str)] = &[
        (&["born", "birth"], "birth", "born", "bio"),
        (&["died", "death", "deceased", "killed"], "death", "died", "bio"),
        (&["married", "marriage", "wed"], "marriage", "married", "bio"),
        (
            &["studied", "graduated", "enrolled", "educated"],
            "education",
            "studied",
            "bio",
        ),
        (
            &["worked", "employed", "served", "appointed"],
            "employment",
            "worked",
            "bio",
        ),
        (
            &["moved", "relocated", "emigrated", "immigrated", "settled", "arrived"],
            "relocation",
            "moved",
            "bio",
        ),
        (
            &["visited", "travelled", "traveled", "toured", "journeyed"],
            "travel",
            "visited",
            "bio",
        ),
        (&["lived", "resided", "stayed"], "residence", "lived", "bio"),
        (&["exiled", "fled"], "exile", "exiled", "bio"),
        (&["imprisoned", "arrested", "jailed"], "imprisonment", "imprisoned", "bio"),
        (
            &["fought", "defeated", "besieged", "invaded", "commanded"],
            "battle",
            "fought",
            "bio",
        ),
        (&["crowned", "elected", "proclaimed", "reigned"], "office", "took office", "bio"),
        (&["spoke", "speech", "addressed", "lectured"], "speech", "spoke", "bio"),
        (
            &["published", "wrote", "authored"],
            "publication",
            "published",
            "work",
        ),
        (
            &["painted", "composed", "sculpted", "filmed", "directed"],
            "creation",
            "created",
            "work",
        ),
        (
            &["discovered", "invented", "proved", "demonstrated"],
            "discovery",
            "discovered",
            "work",
        ),
        (&["awarded", "received", "won", "nobel"], "award", "awarded", "bio"),
    ];
    for (needles, event_type, label, family) in rules {
        if needles.iter().any(|n| verb_token_match(verb, n)) {
            return Some(Classified {
                event_type: (*event_type).into(),
                verb_label: (*label).into(),
                family,
            });
        }
    }
    None
}

/// Whole-token match — avoids `"studied".contains("died")`.
fn verb_token_match(verb: &str, needle: &str) -> bool {
    if verb == needle {
        return true;
    }
    verb.split(|c: char| !c.is_ascii_alphabetic())
        .any(|token| token == needle)
}

fn classify_sentence_cues(sentence_lc: &str) -> Option<Classified> {
    let cues: &[(&[&str], &str, &str, &str)] = &[
        (&["was born", "were born"], "birth", "born", "bio"),
        (&["died in", " died ", "was killed"], "death", "died", "bio"),
        (&["married"], "marriage", "married", "bio"),
        (
            &["studied at", "graduated from", "educated at"],
            "education",
            "studied",
            "bio",
        ),
        (&["lived in exile", "went into exile", "was exiled"], "exile", "exiled", "bio"),
        (&["was crowned", "became emperor", "became king", "became president"], "office", "took office", "bio"),
        (&["battle of", "fought at", "defeated at"], "battle", "fought", "bio"),
        (&["published", "principia", "wrote "], "publication", "published", "work"),
        (&["discovered", "invented"], "discovery", "discovered", "work"),
        (&["moved to", "settled in", "emigrated to"], "relocation", "moved", "bio"),
        (&["visited", "travelled to", "traveled to"], "travel", "visited", "bio"),
        (&["lived in", "resided in"], "residence", "lived", "bio"),
        (&["worked at", "worked in", "served as"], "employment", "worked", "bio"),
        (&["imprisoned", "arrested in"], "imprisonment", "imprisoned", "bio"),
        (&["awarded", "nobel prize", "received the"], "award", "awarded", "bio"),
    ];
    for (needles, event_type, label, family) in cues {
        if needles.iter().any(|n| sentence_lc.contains(n)) {
            return Some(Classified {
                event_type: (*event_type).into(),
                verb_label: (*label).into(),
                family,
            });
        }
    }
    None
}

fn infer_epistemic_status(sentence_lc: &str, family: &str) -> String {
    const RUMOR: &[&str] = &[
        "allegedly",
        "reportedly",
        "rumor",
        "rumour",
        "supposedly",
        "purportedly",
        "it is said",
        "some claim",
        "unconfirmed",
    ];
    const THEORY: &[&str] = &[
        "according to legend",
        "legend has it",
        "historians debate",
        "it has been theorized",
        "one theory",
        "hypothesized",
        "possibly",
        "perhaps",
        "may have",
        "might have",
    ];
    const UNCERTAIN: &[&str] = &[
        "around ",
        "circa ",
        "c. ",
        "approximately",
        "about the year",
        "uncertain",
        "unknown date",
        "date unknown",
    ];

    if RUMOR.iter().any(|c| sentence_lc.contains(c)) {
        return "rumor".into();
    }
    if THEORY.iter().any(|c| sentence_lc.contains(c)) {
        return "theory".into();
    }
    if UNCERTAIN.iter().any(|c| sentence_lc.contains(c)) {
        return "uncertain".into();
    }
    // Legacy commemorations are attested facts about heritage, not lived biography.
    if family == "legacy" {
        return "attested".into();
    }
    if family == "bio" || family == "work" {
        return "established".into();
    }
    "attested".into()
}

fn reject(input: &CandidateInput, score: f64, reason: &str) -> JudgeVerdict {
    JudgeVerdict {
        label: JudgeLabel::Reject,
        score,
        reason: reason.into(),
        event_type: "life_event".into(),
        epistemic_status: "uncertain".into(),
        title: input.person_surface.clone(),
        summary: input.sentence_text.clone(),
        start_time: None,
        time_json: Value::Null,
        place_label: input.place_surface.clone(),
        lat: None,
        lon: None,
        map_eligible: false,
        confidence: score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_birth_candidate() {
        let verdict = judge_candidate(&CandidateInput {
            person_surface: "Alan Turing".into(),
            time_surface: Some("1912".into()),
            place_surface: Some("London".into()),
            verb_pivot: Some("born".into()),
            sentence_text: "Alan Turing was born in 1912 in London.".into(),
        });
        assert_eq!(verdict.label, JudgeLabel::Accept);
        assert_eq!(verdict.event_type, "birth");
        assert_eq!(verdict.epistemic_status, "established");
    }

    #[test]
    fn accepts_cosmos_aux_verb_birth_via_sentence() {
        let verdict = judge_candidate(&CandidateInput {
            person_surface: "Napoleon Bonaparte".into(),
            time_surface: Some("1769".into()),
            place_surface: Some("Ajaccio".into()),
            verb_pivot: Some("was".into()),
            sentence_text: "Napoleon Bonaparte was born in 1769 in Ajaccio.".into(),
        });
        assert_eq!(verdict.label, JudgeLabel::Accept);
        assert_eq!(verdict.event_type, "birth");
    }

    #[test]
    fn classifies_publication_and_battle() {
        let pub_v = judge_candidate(&CandidateInput {
            person_surface: "Isaac Newton".into(),
            time_surface: Some("1687".into()),
            place_surface: Some("London".into()),
            verb_pivot: Some("published".into()),
            sentence_text: "Isaac Newton published the Principia in 1687 in London.".into(),
        });
        assert_eq!(pub_v.event_type, "publication");

        let battle = judge_candidate(&CandidateInput {
            person_surface: "Napoleon Bonaparte".into(),
            time_surface: Some("1815".into()),
            place_surface: Some("Waterloo".into()),
            verb_pivot: Some("fought".into()),
            sentence_text: "Napoleon Bonaparte fought at Waterloo in 1815.".into(),
        });
        assert_eq!(battle.event_type, "battle");
    }

    #[test]
    fn classifies_rumor_and_statue() {
        let rumor = judge_candidate(&CandidateInput {
            person_surface: "Napoleon Bonaparte".into(),
            time_surface: Some("1812".into()),
            place_surface: Some("Moscow".into()),
            verb_pivot: Some("visited".into()),
            sentence_text: "Napoleon Bonaparte allegedly visited Moscow in 1812.".into(),
        });
        assert_eq!(rumor.label, JudgeLabel::Accept);
        assert_eq!(rumor.epistemic_status, "rumor");

        let statue = judge_candidate(&CandidateInput {
            person_surface: "Napoleon Bonaparte".into(),
            time_surface: Some("1865".into()),
            place_surface: Some("Paris".into()),
            verb_pivot: Some("unveiled".into()),
            sentence_text: "A statue of Napoleon Bonaparte was unveiled in 1865 in Paris.".into(),
        });
        assert_eq!(statue.event_type, "statue");
        assert_eq!(statue.epistemic_status, "attested");
    }

    #[test]
    fn studied_is_education_not_death() {
        let verdict = judge_candidate(&CandidateInput {
            person_surface: "Napoleon Bonaparte".into(),
            time_surface: Some("1784".into()),
            place_surface: Some("Brienne".into()),
            verb_pivot: Some("studied".into()),
            sentence_text: "Napoleon Bonaparte studied at the military school in 1784 in Brienne."
                .into(),
        });
        assert_eq!(verdict.event_type, "education");
        assert_ne!(verdict.event_type, "death");
    }

    #[test]
    fn rejects_missing_time() {
        let verdict = judge_candidate(&CandidateInput {
            person_surface: "Alan Turing".into(),
            time_surface: None,
            place_surface: Some("London".into()),
            verb_pivot: Some("born".into()),
            sentence_text: "Alan Turing was born in London.".into(),
        });
        assert_eq!(verdict.label, JudgeLabel::Reject);
    }
}
