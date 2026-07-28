// crates/talaria-text/src/sentences.rs
use crate::wikitext::wikitext_to_plain;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentenceSpan {
    pub ordinal: i32,
    pub text: String,
    pub char_start: i32,
    pub char_end: i32,
}

const MIN_SENTENCE_LEN: usize = 12;

pub fn segment_wikitext(wikitext: &str) -> Vec<SentenceSpan> {
    let plain = wikitext_to_plain(wikitext);
    split_sentences(&plain)
}

pub fn split_sentences(text: &str) -> Vec<SentenceSpan> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }

    let boundaries = sentence_boundaries(text);
    let mut spans = Vec::new();
    let mut start = 0usize;

    for boundary in boundaries {
        push_sentence(&mut spans, text, start, boundary);
        start = boundary;
        while start < text.len() && text.as_bytes()[start] == b' ' {
            start += 1;
        }
    }

    if start < text.len() {
        push_sentence(&mut spans, text, start, text.len());
    }

    spans
}

fn push_sentence(spans: &mut Vec<SentenceSpan>, text: &str, start: usize, end: usize) {
    let slice = text[start..end].trim();
    if slice.len() < MIN_SENTENCE_LEN || !slice.chars().any(|c| c.is_alphabetic()) {
        return;
    }
    spans.push(SentenceSpan {
        ordinal: spans.len() as i32,
        text: slice.to_string(),
        char_start: start as i32,
        char_end: end as i32,
    });
}

fn sentence_boundaries(text: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut boundaries = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        if ch == '.' || ch == '!' || ch == '?' {
            if is_abbreviation_boundary(text, i) {
                i += 1;
                continue;
            }
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && is_sentence_start(text, j) {
                boundaries.push(j);
            }
        }
        i += 1;
    }
    boundaries
}

fn is_sentence_start(text: &str, idx: usize) -> bool {
    let rest = &text[idx..];
    let mut chars = rest.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if first.is_ascii_uppercase() || first.is_ascii_digit() {
        return true;
    }
    if first == '"' || first == '\'' || first == '(' {
        return chars.next().is_some_and(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    }
    false
}

fn is_abbreviation_boundary(text: &str, dot_idx: usize) -> bool {
    let prefix = &text[..=dot_idx];
    const ABBREVS: &[&str] = &[
        "Mr.", "Mrs.", "Ms.", "Dr.", "Prof.", "St.", "Jr.", "Sr.", "vs.", "etc.",
        "e.g.", "i.e.", "U.S.", "U.K.", "b.", "d.", "c.", "fl.", "ca.", "approx.",
    ];
    for abbr in ABBREVS {
        if prefix.ends_with(abbr) {
            return true;
        }
    }

    if dot_idx >= 1 {
        let prev = text.as_bytes()[dot_idx - 1];
        if prev.is_ascii_uppercase() && dot_idx >= 2 {
            let prev2 = text.as_bytes()[dot_idx - 2];
            if prev2 == b' ' || prev2 == b'\n' {
                return true;
            }
        }
        if prev.is_ascii_digit() {
            let digits = digits_before_dot(text, dot_idx);
            if digits >= 4 {
                return false;
            }
            return true;
        }
    }

    false
}

fn digits_before_dot(text: &str, dot_idx: usize) -> usize {
    let mut count = 0usize;
    let mut i = dot_idx;
    while i > 0 && text.as_bytes()[i - 1].is_ascii_digit() {
        count += 1;
        i -= 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_biographical_sentences() {
        let text = "Ada Lovelace was born in 1815. She worked with Charles Babbage on the Analytical Engine. Her notes were visionary.";
        let spans = split_sentences(text);
        assert_eq!(spans.len(), 3);
        assert!(spans[0].text.contains("born in 1815"));
    }

    #[test]
    fn keeps_abbreviations_together() {
        let text = "Dr. Smith was born in London. He moved to Cambridge in 1820.";
        let spans = split_sentences(text);
        assert_eq!(spans.len(), 2);
        assert!(spans[0].text.starts_with("Dr. Smith"));
    }

    #[test]
    fn segments_wikitext_end_to_end() {
        let raw = "'''Alan Turing''' (1912–1954) was a mathematician. He worked at Bletchley Park.";
        let spans = segment_wikitext(raw);
        assert_eq!(spans.len(), 2);
    }
}
