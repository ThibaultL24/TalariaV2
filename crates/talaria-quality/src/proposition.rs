// crates/talaria-quality/src/proposition.rs
//! One extracted proposition, before it is compared with a canonical event.
//!
//! The same sentence found by three extractors is one proposition.
//! Two actions in that sentence stay two propositions when their places differ.

use crate::documentary::{fold, is_bibliographic_quote};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeReading {
    Explicit,
    Inherited,
    Approximate,
    Conflicting,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Novelty {
    New,
    Duplicate,
    Variant,
    AdditionalEvidence,
    Uncertain,
    Rejected,
}

#[derive(Debug, Clone, Copy)]
pub struct KnownEvent<'a> {
    pub id: &'a str,
    pub logical_id: &'a str,
    pub event_type: &'a str,
    pub year: i32,
    pub title: &'a str,
    pub place_label: &'a str,
}

const REALMS: &[&str] = &[
    "sardinia",
    "sardegna",
    "italy",
    "italie",
    "france",
    "austria",
    "autriche",
    "piedmont",
    "piemont",
    "europe",
    "kingdom",
];

pub fn place_stem(place: &str) -> String {
    fold(place)
        .split(|ch: char| !ch.is_alphanumeric())
        .find(|part| !part.is_empty())
        .unwrap_or("")
        .to_string()
}

pub fn is_political_realm(place: &str) -> bool {
    let stem = place_stem(place);
    REALMS.contains(&stem.as_str())
}

/// The place of the action, when the clause names it.
/// A kingdom or an army's origin does not replace that place.
/// Returns None when the only candidate is a political realm.
pub fn anchored_action_place(clause: &str, event_type: &str, proposed: &str) -> Option<String> {
    if !matches!(event_type, "battle" | "siege") {
        let trimmed = proposed.trim();
        return (!trimmed.is_empty()).then(|| trimmed.to_string());
    }
    if let Some(named) = place_after_cues(
        clause,
        &[
            "battle of ",
            "bataille de ",
            "bataille d'",
            "siege of ",
            "siège de ",
            "siège d'",
        ],
    ) {
        return Some(named);
    }
    if let Some(named) = place_after_cues(clause, &[" near ", " près de ", " pres de ", " fought at "])
    {
        if !is_political_realm(&named) {
            return Some(named);
        }
    }
    if is_political_realm(proposed) || proposed.contains('(') {
        return None;
    }
    let trimmed = proposed.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Grouping heuristic for one extraction pass.
/// The historical identity is `occurrence_key`, which also keeps the action and the object.
/// Two events in the same city and the same year stay apart when their clauses differ.
pub fn proposition_key(event_type: &str, year: Option<i32>, place: &str, clause: &str) -> String {
    format!(
        "{}|{}|{}|{}",
        event_type,
        year.unwrap_or(0),
        place_stem(place),
        clause_fingerprint(clause)
    )
}

fn clause_fingerprint(clause: &str) -> String {
    const SKIP: &[&str] = &[
        "the", "and", "was", "were", "with", "from", "that", "for", "his", "her", "its", "les",
        "des", "une",
    ];
    let folded = fold(clause);
    let words = folded
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| word.len() > 2 && !SKIP.contains(word))
        .take(8)
        .collect::<Vec<_>>();
    if words.is_empty() {
        "bare".into()
    } else {
        words.join("-")
    }
}

pub fn time_reading(clause_year: Option<i32>, context_year: Option<i32>, same_subject: bool) -> TimeReading {
    match (clause_year, context_year, same_subject) {
        (Some(own), Some(context), _) if own != context => TimeReading::Conflicting,
        (Some(_), _, _) => TimeReading::Explicit,
        (None, Some(_), true) => TimeReading::Inherited,
        (None, Some(_), false) => TimeReading::Unknown,
        (None, None, _) => TimeReading::Unknown,
    }
}

pub fn novelty_of(
    event_type: &str,
    year: Option<i32>,
    place: &str,
    clause: &str,
    known: &[KnownEvent<'_>],
) -> Novelty {
    if is_bibliographic_quote(clause) {
        return Novelty::Rejected;
    }
    let stem = place_stem(place);
    if stem.is_empty() {
        return Novelty::Uncertain;
    }
    for event in known {
        if event.event_type != event_type {
            continue;
        }
        let named = place_after_cues(
            event.title,
            &["battle of ", "bataille de ", "siege of ", "siège de ", "treaty of ", "armistice of "],
        );
        let known_stem = named
            .as_deref()
            .map(place_stem)
            .filter(|stem| !stem.is_empty())
            .unwrap_or_else(|| place_stem(event.place_label));
        let same_place = stem == known_stem;
        let same_year = year == Some(event.year);
        if same_place && same_year {
            return if statements_match(clause, event.title) {
                Novelty::Duplicate
            } else {
                Novelty::AdditionalEvidence
            };
        }
        if same_place && year.is_none() {
            return Novelty::Uncertain;
        }
        if same_place && year.is_some() {
            return Novelty::Variant;
        }
        if same_year && is_political_realm(place) && known_stem == stem {
            return Novelty::Uncertain;
        }
    }
    if is_political_realm(place) {
        return Novelty::Uncertain;
    }
    Novelty::New
}

fn statements_match(clause: &str, title: &str) -> bool {
    let clause = fold(clause);
    let title = fold(title);
    if clause.is_empty() || title.is_empty() {
        return false;
    }
    let head: String = title.chars().take(42).collect();
    clause.contains(head.trim()) || title.contains(clause.trim())
}

fn place_after_cues(clause: &str, cues: &[&str]) -> Option<String> {
    let lower = clause.to_lowercase();
    for cue in cues {
        let Some(pos) = lower.find(cue) else {
            continue;
        };
        let after = &clause[pos + cue.len()..];
        if let Some(token) = clean_place_token(after) {
            return Some(token);
        }
    }
    None
}

fn clean_place_token(after: &str) -> Option<String> {
    let mut words = Vec::new();
    for word in after.split_whitespace() {
        if word.contains('(') {
            break;
        }
        let clean = word.trim_matches(|ch: char| !ch.is_alphabetic() && ch != '-' && ch != '\'');
        if clean.is_empty() {
            break;
        }
        let lower = clean.to_lowercase();
        if [
            "was", "were", "fought", "between", "under", "near", "on", "in", "and", "the", "de",
            "du",
        ]
        .contains(&lower.as_str())
        {
            break;
        }
        let starts_name = clean.chars().next().is_some_and(|ch| ch.is_uppercase());
        if words.is_empty() || starts_name {
            words.push(clean.to_string());
            if words.len() == 2 {
                break;
            }
            continue;
        }
        break;
    }
    let token = words.join(" ");
    (token.chars().count() >= 2).then_some(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LONATO: KnownEvent<'static> = KnownEvent {
        id: "07764eba-3f83-4a1f-a737-a556caf4ce8f",
        logical_id: "472a66c3-2775-45cd-9660-fcaf7afb70a8",
        event_type: "battle",
        year: 1796,
        title: "The Battle of Lonato was fought on 3 and 4 August 1796",
        place_label: "Italy",
    };

    #[test]
    fn lonato_already_stored_under_italy_is_a_duplicate() {
        let clause = "The Battle of Lonato was fought on 3 and 4 August 1796 between the French Army of Italy and an Austrian corps.";
        assert_eq!(
            novelty_of("battle", Some(1796), "Lonato", clause, &[LONATO]),
            Novelty::Duplicate
        );
    }

    #[test]
    fn sardinia_is_not_the_battlefield_of_montenotte() {
        let clause = "The Battle of Montenotte was fought on 12 April 1796, near Montenotte Superiore (then in the Kingdom of Sardinia, now in Italy).";
        assert_eq!(
            anchored_action_place(clause, "battle", "Sardinia").as_deref(),
            Some("Montenotte")
        );
        assert_eq!(
            proposition_key("battle", Some(1796), "Montenotte", clause),
            proposition_key("battle", Some(1796), "Montenotte Superiore", clause)
        );
    }

    #[test]
    fn two_meetings_in_paris_in_the_same_year_stay_apart() {
        let josephine = proposition_key(
            "meeting",
            Some(1796),
            "Paris",
            "He met Josephine in Paris in 1796.",
        );
        let barras = proposition_key(
            "meeting",
            Some(1796),
            "Paris",
            "He met Barras in Paris in 1796.",
        );
        assert_ne!(josephine, barras);
    }

    #[test]
    fn a_second_sentence_about_lonato_is_more_evidence() {
        let clause = "Bonaparte crossed the bridge at Lonato the next morning.";
        assert_eq!(
            novelty_of("battle", Some(1796), "Lonato", clause, &[LONATO]),
            Novelty::AdditionalEvidence
        );
    }

    #[test]
    fn a_journey_to_sardinia_keeps_the_island() {
        let clause = "He traveled to Sardinia in the hopes of reprocessing the slag.";
        assert_eq!(
            anchored_action_place(clause, "travel", "Sardinia").as_deref(),
            Some("Sardinia")
        );
    }

    #[test]
    fn an_inherited_year_is_not_an_explicit_date() {
        assert_eq!(time_reading(None, Some(1855), true), TimeReading::Inherited);
        assert_eq!(
            time_reading(Some(1862), Some(1855), true),
            TimeReading::Conflicting
        );
        assert_eq!(time_reading(None, Some(1855), false), TimeReading::Unknown);
    }

    #[test]
    fn an_undated_bassano_is_not_a_second_battle() {
        let known = KnownEvent {
            id: "stored-bassano",
            logical_id: "stored-bassano",
            event_type: "battle",
            year: 1796,
            title: "The Battle of Bassano was fought on 8 September 1796",
            place_label: "Bassano",
        };
        assert_eq!(
            novelty_of("battle", None, "Bassano", "Battle of Bassano", &[known]),
            Novelty::Uncertain
        );
    }

    #[test]
    fn castiglione_is_new_when_lonato_is_the_only_known_battle() {
        assert_eq!(
            novelty_of("battle", Some(1796), "Castiglione", "Battle of Castiglione", &[LONATO]),
            Novelty::New
        );
    }
}
