// crates/talaria-quality/src/explorer.rs
//! What the explorer may show as an event: a dated occurrence, not a QID stub.

use crate::gates::event_type_is_map_locus;

pub fn is_wikidata_qid(value: &str) -> bool {
    let value = value.trim();
    value.len() >= 2
        && value.starts_with('Q')
        && value[1..].bytes().all(|b| b.is_ascii_digit())
}

pub fn is_statement_blob(text: &str) -> bool {
    let text = text.trim();
    text.starts_with("STATEMENT")
        || (text.contains(" | ") && text.matches('|').count() >= 2)
}

pub fn is_human_place_label(place: &str) -> bool {
    let place = place.trim();
    if place.is_empty() || is_wikidata_qid(place) {
        return false;
    }
    if place.chars().count() > 36 || place.contains('.') {
        return false;
    }
    place.split_whitespace().count() <= 4
}

pub fn is_occurrence_prose(text: &str) -> bool {
    let text = text.trim();
    if text.chars().count() < 28 || is_statement_blob(text) {
        return false;
    }
    if matches!(
        text.to_ascii_lowercase().as_str(),
        "born_in" | "died_in" | "resided_in" | "held_office" | "occurred"
    ) {
        return false;
    }
    text.contains(' ')
}

fn place_mentioned(place: &str, text: &str) -> bool {
    let hay = text.to_ascii_lowercase();
    let needle = place.to_ascii_lowercase();
    if hay.contains(&needle) {
        return true;
    }
    needle
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| word.chars().count() >= 4)
        .any(|word| hay.contains(word))
}

fn type_cues(event_type: &str, text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    match event_type {
        "birth" => text.contains("born") || text.contains("naît") || text.contains("nait"),
        "death" => {
            text.contains("died") || text.contains("mort") || text.contains("décès")
        }
        "marriage" | "wedding" => {
            text.contains("married") || text.contains("marié") || text.contains("wedding")
        }
        "battle" | "siege" => [
            "battle",
            "fought",
            "siege",
            "bataille",
            "combat",
            "defeated",
        ]
        .iter()
        .any(|cue| text.contains(cue)),
        "residence" => ["lived", "resided", "moved", "stayed", "exiled", "habita"]
            .iter()
            .any(|cue| text.contains(cue)),
        "education" => ["studied", "school", "université", "college", "étudiant"]
            .iter()
            .any(|cue| text.contains(cue)),
        "office" => ["elected", "president", "king", "inaugur", "crowned", "empereur"]
            .iter()
            .any(|cue| text.contains(cue)),
        "diplomatic" => ["treaty", "accord", "negotiat", "diplom", "signed the"]
            .iter()
            .any(|cue| text.contains(cue)),
        "travel" | "arrival" | "departure" | "passage" => {
            ["visited", "arrived", "left", "travel", "voyage"].iter().any(|cue| text.contains(cue))
        }
        _ => true,
    }
}

/// Map pin: locus type, named place, and a sentence that is actually that event.
pub fn is_explorer_map_event(
    event_type: &str,
    place: Option<&str>,
    summary: Option<&str>,
    title: Option<&str>,
) -> bool {
    if !event_type_is_map_locus(event_type) {
        return false;
    }
    let Some(place) = place.map(str::trim).filter(|text| !text.is_empty()) else {
        return false;
    };
    if !is_human_place_label(place) {
        return false;
    }
    let body = summary.or(title).unwrap_or("");
    if !is_occurrence_prose(body) {
        return false;
    }
    let life = matches!(event_type, "birth" | "death" | "marriage" | "wedding")
        && type_cues(event_type, body);
    if life {
        return true;
    }
    place_mentioned(place, body) && type_cues(event_type, body)
}

pub fn is_explorer_timeline_event(
    event_type: &str,
    place: Option<&str>,
    summary: Option<&str>,
    title: Option<&str>,
) -> bool {
    if matches!(event_type, "birth" | "death") {
        return true;
    }
    let body = summary.or(title).unwrap_or("");
    if matches!(event_type, "marriage" | "wedding")
        && is_occurrence_prose(body)
        && type_cues(event_type, body)
    {
        return true;
    }
    is_explorer_map_event(event_type, place, summary, title)
}

fn first_sentence(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    let cut = trimmed
        .find(". ")
        .or_else(|| trimmed.find('。'))
        .unwrap_or(trimmed.len());
    let mut sentence = trimmed[..cut].trim().trim_end_matches('.').to_string();
    if sentence.chars().count() > max_chars {
        sentence = sentence.chars().take(max_chars).collect::<String>();
        sentence = format!("{}…", sentence.trim_end());
    }
    sentence
}

pub fn explorer_summary<'a>(quote: Option<&'a str>, summary: Option<&'a str>) -> Option<String> {
    for text in [quote, summary].into_iter().flatten() {
        if is_occurrence_prose(text) {
            return Some(text.trim().to_string());
        }
    }
    None
}

pub fn explorer_headline(
    person: &str,
    event_type: &str,
    year: Option<i32>,
    place: Option<&str>,
    quote: Option<&str>,
    summary: Option<&str>,
) -> String {
    if let Some(prose) = explorer_summary(quote, summary) {
        return first_sentence(&prose, 140);
    }
    if event_type == "birth" {
        if let Some(year) = year {
            return format!("Born in {year}");
        }
    }
    if event_type == "death" {
        if let Some(year) = year {
            return format!("Died in {year}");
        }
    }
    let mut parts = vec![event_type.replace('_', " ")];
    if let Some(year) = year {
        parts.push(year.to_string());
    }
    if let Some(place) = place.filter(|p| is_human_place_label(p)) {
        parts.push(place.to_string());
    }
    if parts.len() == 1 {
        format!("{person} · {}", parts[0])
    } else {
        parts.join(" · ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qid_and_clause_places_are_not_human() {
        assert!(is_wikidata_qid("Q127421251"));
        assert!(!is_human_place_label("Q35525"));
        assert!(!is_human_place_label(
            "citizens from six Muslim-majority countries for four months"
        ));
        assert!(is_human_place_label("Paris"));
        assert!(is_human_place_label("New York"));
    }

    #[test]
    fn statement_and_predicates_are_not_prose() {
        assert!(!is_occurrence_prose("Trump | notable_event | occurred | 2024 | Q127421251"));
        assert!(!is_occurrence_prose("born_in"));
        assert!(!is_occurrence_prose("married Q432473"));
        assert!(is_occurrence_prose(
            "In 1977, Trump married Ivana Zelníčková"
        ));
    }

    #[test]
    fn junk_map_points_are_hidden() {
        assert!(!is_explorer_map_event(
            "notable_event",
            Some("Q127421251"),
            Some("Trump | notable_event | occurred | 2024 | Q127421251"),
            None,
        ));
        assert!(!is_explorer_map_event(
            "residence",
            Some("Q35525"),
            Some("Donald Trump — sources situate this episode in Q35525 in 2025."),
            None,
        ));
        assert!(!is_explorer_map_event(
            "historical_fact",
            Some("Paris"),
            Some("Trump frequently threatened and enacted tariffs against treaty allies"),
            None,
        ));
        assert!(!is_explorer_map_event(
            "military_campaign",
            Some("Latin America"),
            Some("Trump began his second presidency by initiating mass layoffs of federal workers."),
            None,
        ));
        assert!(!is_explorer_map_event(
            "battle",
            Some("New York"),
            Some("On July 13, 2024, Trump was shot in the ear in an assassination attempt at a campaign rally in Butler"),
            None,
        ));
    }

    #[test]
    fn real_life_events_stay() {
        assert!(is_explorer_map_event(
            "marriage",
            Some("Siena"),
            Some("In 1977, Trump married Ivana Zelníčková"),
            None,
        ));
        assert!(is_explorer_map_event(
            "battle",
            Some("Waterloo"),
            Some("Napoleon fought and was defeated at Waterloo in 1815."),
            None,
        ));
        assert!(is_explorer_timeline_event(
            "birth",
            None,
            Some("born_in"),
            Some("Donald Trump — birth (1946)"),
        ));
    }

    #[test]
    fn headline_prefers_the_sentence() {
        assert_eq!(
            explorer_headline(
                "Donald Trump",
                "marriage",
                Some(1977),
                Some("Siena"),
                None,
                Some("In 1977, Trump married Ivana Zelníčková"),
            ),
            "In 1977, Trump married Ivana Zelníčková"
        );
        assert_eq!(
            explorer_headline(
                "Donald Trump",
                "notable_event",
                Some(2024),
                Some("Q127421251"),
                None,
                Some("Trump | notable_event | occurred | 2024 | Q127421251"),
            ),
            "notable event · 2024"
        );
    }
}
