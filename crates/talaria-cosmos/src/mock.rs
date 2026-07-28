// crates/talaria-cosmos/src/mock.rs
use crate::{BatchInputItem, BatchOutputItem, ExtractedTuple};

/// Lightweight rule-based extractor for dev/CI when COSMOS models are unavailable.
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
    let lower = text.to_lowercase();
    let mut tuples = Vec::new();

    if let Some(tuple) = match_born_in(text, &lower) {
        tuples.push(tuple);
    }

    tuples
}

fn match_born_in(text: &str, lower: &str) -> Option<ExtractedTuple> {
    let born_idx = lower.find(" was born in ")?;
    let person = text[..born_idx].trim().trim_start_matches('"').to_string();
    let rest = &text[born_idx + " was born in ".len()..];
    let mut parts = rest.splitn(3, ' ');
    let time = parts.next()?.trim_end_matches('.').to_string();
    if parts.next()? != "in" {
        return None;
    }
    let place = parts.next()?.trim_end_matches('.').to_string();

    if person.is_empty() || time.is_empty() || place.is_empty() {
        return None;
    }

    Some(ExtractedTuple {
        person,
        time,
        place,
        verb: Some("born".into()),
    })
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
    }
}
