// crates/talaria-ai/src/passages.rs
//! Split a collected document into citable passages. A passage is kept as
//! documentation unless the quote, the year, and the event type are all in it.

use sha2::{Digest, Sha256};

use crate::quote_is_documentary;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passage {
    pub fragment_id: String,
    pub start: usize,
    pub end: usize,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifiedPassage {
    pub event_type: String,
    pub predicate: String,
    pub year: i32,
    pub place: Option<String>,
    pub quote: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PassageDecision {
    Candidate(QualifiedPassage),
    Documentation { reason: &'static str },
    AttachmentError,
}

const PASSAGE_LIMIT: usize = 24;
const PASSAGE_CHARS: usize = 1_200;

pub fn segment_passages(source_uri: &str, text: &str) -> Vec<Passage> {
    let mut passages = Vec::new();
    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];
        let next = rest.find("\n\n").unwrap_or(rest.len());
        push_paragraph(&mut passages, source_uri, text, index, &rest[..next]);
        index += next;
        if index < text.len() {
            index += 2;
        }
    }
    passages.sort_by_key(|passage| years_in(&passage.text).is_empty());
    passages.truncate(PASSAGE_LIMIT);
    passages.sort_by_key(|passage| passage.start);
    passages
}

pub fn qualify_passage(subject: &str, passage: &Passage) -> PassageDecision {
    if !quote_is_documentary(&passage.text, &passage.text) {
        return PassageDecision::Documentation {
            reason: "quote_not_in_source",
        };
    }
    if !subject_in_quote(subject, &passage.text) {
        return PassageDecision::AttachmentError;
    }
    let Some(year) = years_in(&passage.text).into_iter().next() else {
        return PassageDecision::Documentation {
            reason: "date_not_in_passage",
        };
    };
    let Some((event_type, predicate)) = demonstrated_type(&passage.text) else {
        return PassageDecision::Documentation {
            reason: "type_not_demonstrated",
        };
    };
    let place = place_in_quote(&passage.text);
    if event_type == "coronation" && place.is_none() {
        return PassageDecision::Documentation {
            reason: "coronation_place_unverified",
        };
    }
    PassageDecision::Candidate(QualifiedPassage {
        event_type: event_type.to_string(),
        predicate: predicate.to_string(),
        year,
        place,
        quote: passage.text.clone(),
    })
}

pub fn years_in(text: &str) -> Vec<i32> {
    let bytes = text.as_bytes();
    let mut years = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let negative =
            bytes[index] == b'-' && bytes.get(index + 1).is_some_and(|c| c.is_ascii_digit());
        let start = if negative { index + 1 } else { index };
        if bytes.get(start).is_some_and(|c| c.is_ascii_digit()) {
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            let width = end - start;
            let before_ok = index == 0 || !bytes[index - 1].is_ascii_alphanumeric();
            let after_ok = end == bytes.len() || !bytes[end].is_ascii_alphanumeric();
            if (3..=4).contains(&width) && before_ok && after_ok {
                if let Ok(value) = text[start..end].parse::<i32>() {
                    let year = if negative { -value } else { value };
                    if (-5_000..=2_100).contains(&year) && year != 0 {
                        years.push(year);
                    }
                }
            }
            index = end;
            continue;
        }
        index += 1;
    }
    years
}

fn push_paragraph(
    passages: &mut Vec<Passage>,
    source_uri: &str,
    text: &str,
    offset: usize,
    paragraph: &str,
) {
    let Some(local) = paragraph.find(paragraph.trim()) else {
        return;
    };
    let trimmed = paragraph.trim();
    if trimmed.chars().count() < 40 {
        return;
    }
    let mut cursor = offset + local;
    let mut remaining = trimmed;
    while !remaining.is_empty() {
        let take = window(remaining);
        let end = cursor + take.len();
        passages.push(Passage {
            fragment_id: fragment_id(source_uri, cursor, end),
            start: cursor,
            end,
            text: text[cursor..end].to_string(),
        });
        if take.len() == remaining.len() {
            break;
        }
        cursor = end;
        remaining = &remaining[take.len()..];
    }
}

fn window(text: &str) -> &str {
    if text.chars().count() <= PASSAGE_CHARS {
        return text;
    }
    let mut count = 0;
    let mut split = text.len();
    for (index, character) in text.char_indices() {
        count += 1;
        if count > PASSAGE_CHARS {
            break;
        }
        split = index + character.len_utf8();
    }
    let head = &text[..split];
    head.rfind(". ")
        .map(|index| &head[..index + 1])
        .unwrap_or(head)
}

fn fragment_id(source_uri: &str, start: usize, end: usize) -> String {
    let digest = hex::encode(Sha256::digest(
        format!("{source_uri}:{start}:{end}").as_bytes(),
    ));
    digest[..16].to_string()
}

fn subject_in_quote(subject: &str, quote: &str) -> bool {
    let quote = fold_accents(quote);
    let subject = fold_accents(subject);
    if subject.chars().count() >= 4 && quote.contains(&subject) {
        return true;
    }
    subject.split_whitespace().any(|token| {
        token.chars().count() >= 4
            && !matches!(token, "van" | "von" | "de" | "du" | "des" | "the" | "and")
            && quote.contains(token)
    })
}

fn fold_accents(text: &str) -> String {
    text.chars()
        .map(|character| match character {
            'à' | 'á' | 'â' | 'ä' | 'À' | 'Á' | 'Â' | 'Ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'ï' | 'î' | 'ì' | 'Ï' | 'Î' | 'Ì' => 'i',
            'ô' | 'ö' | 'ò' | 'Ô' | 'Ö' | 'Ò' => 'o',
            'ù' | 'û' | 'ü' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

fn demonstrated_type(quote: &str) -> Option<(&'static str, &'static str)> {
    let lower = quote.to_lowercase();
    if lower.contains("couronnement") || lower.contains("coronation") || lower.contains("crowned") {
        Some(("coronation", "crowned"))
    } else if lower.contains("abdication") || lower.contains("abdiqua") {
        Some(("abdication", "abdicated"))
    } else if lower.contains("bataille") || lower.contains("battle") {
        Some(("battle", "fought"))
    } else {
        None
    }
}

fn place_in_quote(quote: &str) -> Option<String> {
    let lower = quote.to_lowercase();
    for marker in [" à ", " in ", " at "] {
        let Some(index) = lower.find(marker) else {
            continue;
        };
        let Some(rest) = quote.get(index + marker.len()..) else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphabetic() || *c == '-' || *c == '\'')
            .collect();
        if name.chars().count() >= 3 && name.chars().next().is_some_and(|c| c.is_uppercase()) {
            return Some(name);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_text_is_segmented_with_offsets_instead_of_a_prefix() {
        let letter = format!(
            "Napoléon écrit de Paris en 1804.\n\nLe couronnement de Napoléon a lieu à Paris en 1804, devant le pape.\n\n{}",
            "suite ".repeat(4_000)
        );
        let passages = segment_passages("https://fr.wikisource.org/wiki/Lettre", &letter);
        assert!(passages.len() > 1);
        assert!(passages.len() <= PASSAGE_LIMIT);
        let coronation = passages
            .iter()
            .find(|passage| passage.text.contains("couronnement"))
            .unwrap();
        assert_eq!(&letter[coronation.start..coronation.end], coronation.text);
        assert!(!coronation.fragment_id.is_empty());
    }

    #[test]
    fn a_coronation_needs_the_quote_the_year_and_the_place() {
        let passage = Passage {
            fragment_id: "frag".into(),
            start: 0,
            end: 70,
            text: "Le couronnement de Napoléon a lieu à Paris en 1804.".into(),
        };
        let decision = qualify_passage("Napoléon", &passage);
        let PassageDecision::Candidate(found) = decision else {
            panic!("expected a candidate");
        };
        assert_eq!(found.event_type, "coronation");
        assert_eq!(found.year, 1804);
        assert_eq!(found.place.as_deref(), Some("Paris"));
    }

    #[test]
    fn a_quote_without_a_demonstrated_type_stays_documentation() {
        let passage = Passage {
            fragment_id: "frag".into(),
            start: 0,
            end: 40,
            text: "Napoléon écrit à Joseph le 12 mai 1804.".into(),
        };
        assert_eq!(
            qualify_passage("Napoléon", &passage),
            PassageDecision::Documentation {
                reason: "type_not_demonstrated"
            }
        );
    }

    #[test]
    fn a_quote_about_someone_else_is_an_attachment_error() {
        let passage = Passage {
            fragment_id: "frag".into(),
            start: 0,
            end: 48,
            text: "Le couronnement de Charles a lieu à Reims en 1775.".into(),
        };
        assert_eq!(
            qualify_passage("Napoléon", &passage),
            PassageDecision::AttachmentError
        );
        let same = Passage {
            fragment_id: "frag".into(),
            start: 0,
            end: 52,
            text: "Le couronnement de Napoléon a lieu à Paris en 1804.".into(),
        };
        assert!(matches!(
            qualify_passage("Napoleon", &same),
            PassageDecision::Candidate(_)
        ));
    }
}
