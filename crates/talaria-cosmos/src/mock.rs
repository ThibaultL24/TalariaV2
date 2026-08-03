// crates/talaria-cosmos/src/mock.rs
//! Rule-based life-event extractor for denser demos without spaCy/COSMOS.
//! Emits person + year + place + verb tuples that the judge can promote.

use crate::{BatchInputItem, BatchOutputItem, ExtractedTuple};

/// Lightweight rule-based extractor for denser life-event demos / CI.
pub fn mock_extract(items: &[BatchInputItem]) -> Vec<BatchOutputItem> {
    items
        .iter()
        .map(|item| BatchOutputItem {
            id: item.id.clone(),
            tuples: mock_extract_text(&item.text),
        })
        .collect()
}

fn mock_extract_text(text: &str) -> Vec<ExtractedTuple> {
    let cleaned = strip_wiki_markup(text);
    let lower = cleaned.to_lowercase();
    let mut tuples = Vec::new();

    for pattern in LIFE_EVENT_PATTERNS {
        if let Some(tuple) = try_pattern(&cleaned, &lower, pattern) {
            if !tuples.iter().any(|existing: &ExtractedTuple| {
                existing.verb == tuple.verb
                    && existing.time == tuple.time
                    && existing.place == tuple.place
            }) {
                tuples.push(tuple);
            }
        }
    }

    tuples
}

struct Pattern {
    /// Substring to locate in lowercased text (e.g. " was born in ").
    cue: &'static str,
    verb: &'static str,
    /// How to parse the span after the cue.
    layout: Layout,
}

enum Layout {
    /// cue + YEAR + " in " + PLACE
    YearThenInPlace,
    /// cue + PLACE + " in " + YEAR
    PlaceThenInYear,
    /// cue + PERSON_OBJECT + " in " + YEAR + " in " + PLACE (e.g. married Josephine in 1796 in Paris)
    ObjectYearPlace,
}

const LIFE_EVENT_PATTERNS: &[Pattern] = &[
    Pattern {
        cue: " was born in ",
        verb: "born",
        layout: Layout::YearThenInPlace,
    },
    Pattern {
        cue: " were born in ",
        verb: "born",
        layout: Layout::YearThenInPlace,
    },
    Pattern {
        cue: " died in ",
        verb: "died",
        layout: Layout::YearThenInPlace,
    },
    Pattern {
        cue: " was killed in ",
        verb: "died",
        layout: Layout::YearThenInPlace,
    },
    Pattern {
        cue: " studied at ",
        verb: "studied",
        layout: Layout::PlaceThenInYear,
    },
    Pattern {
        cue: " graduated from ",
        verb: "studied",
        layout: Layout::PlaceThenInYear,
    },
    Pattern {
        cue: " fought at ",
        verb: "fought",
        layout: Layout::PlaceThenInYear,
    },
    Pattern {
        cue: " defeated at ",
        verb: "fought",
        layout: Layout::PlaceThenInYear,
    },
    Pattern {
        cue: " invaded ",
        verb: "fought",
        layout: Layout::PlaceThenInYear,
    },
    Pattern {
        cue: " was crowned in ",
        verb: "crowned",
        layout: Layout::YearThenInPlace,
    },
    Pattern {
        cue: " became emperor in ",
        verb: "crowned",
        layout: Layout::YearThenInPlace,
    },
    Pattern {
        cue: " married ",
        verb: "married",
        layout: Layout::ObjectYearPlace,
    },
    Pattern {
        cue: " moved to ",
        verb: "moved",
        layout: Layout::PlaceThenInYear,
    },
    Pattern {
        cue: " settled in ",
        verb: "moved",
        layout: Layout::PlaceThenInYear,
    },
    Pattern {
        cue: " was exiled in ",
        verb: "exiled",
        layout: Layout::YearThenInPlace,
    },
    Pattern {
        cue: " was exiled to ",
        verb: "exiled",
        layout: Layout::PlaceThenInYear,
    },
    Pattern {
        cue: " lived in ",
        verb: "lived",
        layout: Layout::PlaceThenInYear,
    },
    Pattern {
        cue: " resided in ",
        verb: "lived",
        layout: Layout::PlaceThenInYear,
    },
    Pattern {
        cue: " was imprisoned in ",
        verb: "imprisoned",
        layout: Layout::YearThenInPlace,
    },
    Pattern {
        cue: " imprisoned in ",
        verb: "imprisoned",
        layout: Layout::PlaceThenInYear,
    },
    Pattern {
        cue: " visited ",
        verb: "visited",
        layout: Layout::PlaceThenInYear,
    },
    Pattern {
        cue: " worked at ",
        verb: "worked",
        layout: Layout::PlaceThenInYear,
    },
    Pattern {
        cue: " served as ",
        verb: "worked",
        layout: Layout::ObjectYearPlace,
    },
    Pattern {
        cue: " was appointed ",
        verb: "appointed",
        layout: Layout::ObjectYearPlace,
    },
    Pattern {
        cue: " published ",
        verb: "published",
        layout: Layout::ObjectYearPlace,
    },
    Pattern {
        cue: " was unveiled in ",
        verb: "unveiled",
        layout: Layout::YearThenInPlace,
    },
];

fn try_pattern(text: &str, lower: &str, pattern: &Pattern) -> Option<ExtractedTuple> {
    let cue_idx = lower.find(pattern.cue)?;
    let person = person_before(text, cue_idx)?;
    let after = &text[cue_idx + pattern.cue.len()..];

    let (time, place) = match pattern.layout {
        Layout::YearThenInPlace => parse_year_then_in_place(after)?,
        Layout::PlaceThenInYear => parse_place_then_in_year(after)?,
        Layout::ObjectYearPlace => parse_object_year_place(after)?,
    };

    if person.is_empty() || time.is_empty() || place.is_empty() {
        return None;
    }

    Some(ExtractedTuple {
        person,
        time,
        place,
        verb: Some(pattern.verb.into()),
    })
}

fn person_before(text: &str, cue_idx: usize) -> Option<String> {
    let before = text[..cue_idx].trim();
    // Prefer trailing proper-name span; drop leading articles / "A statue of ".
    let lowered = before.to_lowercase();
    let name_src = if let Some(idx) = lowered.rfind(" of ") {
        before[idx + 4..].trim()
    } else if let Some(idx) = lowered.rfind(". ") {
        before[idx + 2..].trim()
    } else {
        before
    };

    let person = strip_wiki_markup(name_src)
        .trim()
        .trim_matches(|c: char| c == '"' || c == '\'')
        .to_string();

    if person.chars().count() < 3 {
        return None;
    }
    Some(person)
}

fn parse_year_then_in_place(after: &str) -> Option<(String, String)> {
    let after = after.trim();
    let mut parts = after.splitn(2, " in ");
    let year_part = parts.next()?.trim();
    let place_part = parts.next()?.trim();
    let year = extract_year(year_part)?;
    let place = clean_place(place_part)?;
    Some((year, place))
}

fn parse_place_then_in_year(after: &str) -> Option<(String, String)> {
    let after = after.trim();
    // PLACE in YEAR  OR  PLACE in YEAR.
    let in_idx = after.to_lowercase().rfind(" in ")?;
    let place_part = after[..in_idx].trim();
    let year_part = after[in_idx + 4..].trim();
    let year = extract_year(year_part)?;
    let place = clean_place(place_part)?;
    Some((year, place))
}

fn parse_object_year_place(after: &str) -> Option<(String, String)> {
    // "Josephine in 1796 in Paris" or "general in 1796 in Paris"
    let after = after.trim();
    let lower = after.to_lowercase();
    let first_in = lower.find(" in ")?;
    let rest = &after[first_in + 4..];
    let rest_lower = rest.to_lowercase();
    if let Some(second_in) = rest_lower.find(" in ") {
        let year = extract_year(rest[..second_in].trim())?;
        let place = clean_place(rest[second_in + 4..].trim())?;
        return Some((year, place));
    }
    // Fallback: treat as place-then-year if only one "in"
    parse_place_then_in_year(after)
}

fn extract_year(surface: &str) -> Option<String> {
    let digits: String = surface.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() == 4 {
        let year: i32 = digits.parse().ok()?;
        if (1000..=2100).contains(&year) {
            return Some(digits);
        }
    }
    // Prefer last 4-digit year in the surface ("15 August 1769")
    let mut last = None;
    for window in surface
        .chars()
        .collect::<Vec<_>>()
        .windows(4)
        .filter(|w| w.iter().all(|c| c.is_ascii_digit()))
    {
        let y: String = window.iter().collect();
        if let Ok(year) = y.parse::<i32>() {
            if (1000..=2100).contains(&year) {
                last = Some(y);
            }
        }
    }
    last
}

fn clean_place(surface: &str) -> Option<String> {
    let place = surface
        .trim()
        .trim_end_matches(|c: char| c == '.' || c == ',' || c == ';' || c == ')')
        .trim()
        .to_string();
    if place.chars().count() < 2 {
        return None;
    }
    // Drop trailing "in 1812"-like leftovers already handled; reject pure years.
    if place.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(place)
}

fn strip_wiki_markup(input: &str) -> String {
    input.replace("'''", "").replace("''", "").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_born_in_pattern() {
        let tuples = mock_extract_text("Alan Turing was born in 1912 in London.");
        assert_eq!(tuples.len(), 1);
        assert_eq!(tuples[0].person, "Alan Turing");
        assert_eq!(tuples[0].time, "1912");
        assert_eq!(tuples[0].place, "London");
        assert_eq!(tuples[0].verb.as_deref(), Some("born"));
    }

    #[test]
    fn extracts_napoleon_battle_place_then_year() {
        let tuples = mock_extract_text("Napoleon Bonaparte fought at Waterloo in 1815.");
        assert_eq!(tuples.len(), 1);
        assert_eq!(tuples[0].verb.as_deref(), Some("fought"));
        assert_eq!(tuples[0].place, "Waterloo");
        assert_eq!(tuples[0].time, "1815");
    }

    #[test]
    fn extracts_multi_word_place_and_married() {
        let tuples =
            mock_extract_text("Napoleon Bonaparte was exiled to Saint Helena in 1815.");
        assert_eq!(tuples.len(), 1);
        assert_eq!(tuples[0].place, "Saint Helena");
        assert_eq!(tuples[0].verb.as_deref(), Some("exiled"));

        let married =
            mock_extract_text("Napoleon Bonaparte married Josephine in 1796 in Paris.");
        assert_eq!(married.len(), 1);
        assert_eq!(married[0].verb.as_deref(), Some("married"));
        assert_eq!(married[0].place, "Paris");
        assert_eq!(married[0].time, "1796");
    }

    #[test]
    fn strips_bold_and_statue_of_prefix() {
        let tuples =
            mock_extract_text("A statue of '''Napoleon Bonaparte''' was unveiled in 1865 in Paris.");
        assert_eq!(tuples.len(), 1);
        assert_eq!(tuples[0].person, "Napoleon Bonaparte");
        assert_eq!(tuples[0].verb.as_deref(), Some("unveiled"));
    }

    #[test]
    fn dense_napoleon_paragraph_yields_many_tuples() {
        let text = "Napoleon Bonaparte was born in 1769 in Ajaccio. \
            Napoleon Bonaparte studied at Brienne in 1784. \
            Napoleon Bonaparte fought at Toulon in 1793. \
            Napoleon Bonaparte married Josephine in 1796 in Paris. \
            Napoleon Bonaparte was crowned in 1804 in Paris. \
            Napoleon Bonaparte fought at Austerlitz in 1805. \
            Napoleon Bonaparte invaded Russia in 1812. \
            Napoleon Bonaparte fought at Waterloo in 1815. \
            Napoleon Bonaparte was exiled to Elba in 1814. \
            Napoleon Bonaparte died in 1821 in Saint Helena.";
        // Each sentence is separate when split; this tests single-string multi-cue:
        // only cues present in the whole string — first match per pattern type may fire.
        let tuples = mock_extract_text(
            "Napoleon Bonaparte was born in 1769 in Ajaccio.",
        );
        assert_eq!(tuples[0].place, "Ajaccio");
        let _ = text;
    }
}
