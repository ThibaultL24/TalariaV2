// crates/talaria-quality/src/grounding.rs
//! Ground LLM extracts in stored document text. No invented quotes.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    Fact,
    Debate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawExtractItem {
    pub lane: String,
    pub event_type: String,
    pub role: String,
    pub year: Option<i32>,
    pub place_surface: Option<String>,
    pub summary: String,
    pub quoted_text: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroundedItem {
    pub lane: Lane,
    pub event_type: String,
    pub role: String,
    pub year: Option<i32>,
    pub place_surface: Option<String>,
    pub summary: String,
    pub quoted_text: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectReason {
    QuoteNotInDocument,
    OtherPersonAgent,
    EmptyQuote,
}

fn fold_ws(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// True when `quote` appears in `document` (whitespace / case insensitive).
pub fn quote_is_grounded(document: &str, quote: &str) -> bool {
    let q = fold_ws(quote);
    if q.is_empty() {
        return false;
    }
    fold_ws(document).contains(&q)
}

fn subject_parts(subject: &str) -> (String, String) {
    let subject = subject.split('(').next().unwrap_or(subject).trim();
    let lower = subject.to_lowercase();
    let surname = subject
        .split_whitespace()
        .last()
        .unwrap_or(subject)
        .to_lowercase();
    (lower, surname)
}

const SKIP_NAME: &[&str] = &[
    "the", "and", "for", "with", "from", "that", "this", "april", "june", "july",
    "march", "august", "january", "february", "september", "october", "november",
    "december", "on", "in", "at", "of", "to", "by",
];

fn looks_like_name_token(tok: &str) -> bool {
    let t = tok.trim_matches(|c: char| !c.is_alphabetic() && c != '-' && c != '\'');
    if t.chars().count() < 3 {
        return false;
    }
    if SKIP_NAME.contains(&t.to_lowercase().as_str()) {
        return false;
    }
    t.chars().next().is_some_and(char::is_uppercase)
}

fn clean_token(tok: &str) -> String {
    tok.trim_matches(|c: char| !c.is_alphabetic() && c != '-' && c != '\'')
        .to_string()
}

fn is_year_or_day(tok: &str) -> bool {
    tok.trim_matches(|c: char| !c.is_ascii_digit())
        .parse::<i32>()
        .is_ok()
}

/// First proper-name span; later place names (Warsaw, Paris) are ignored.
pub fn agent_is_other_person(quote: &str, subject: &str) -> bool {
    let (subject_l, surname) = subject_parts(subject);
    if surname.is_empty() {
        return false;
    }
    let tokens: Vec<&str> = quote.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        let raw = tokens[i];
        let cleaned = clean_token(raw);
        if cleaned.is_empty()
            || is_year_or_day(raw)
            || SKIP_NAME.contains(&cleaned.to_lowercase().as_str())
            || !looks_like_name_token(raw)
        {
            i += 1;
            continue;
        }
        let mut parts = vec![cleaned];
        let mut j = i + 1;
        while j < tokens.len() && looks_like_name_token(tokens[j]) {
            parts.push(clean_token(tokens[j]));
            j += 1;
        }
        let name = parts.join(" ").to_lowercase();
        if subject_l.contains(&name) || name.contains(&subject_l) {
            return false;
        }
        if parts.iter().any(|w| w.to_lowercase() == surname) {
            return false;
        }
        return true;
    }
    false
}

pub fn parse_lane(raw: &str) -> Lane {
    match raw.trim().to_ascii_lowercase().as_str() {
        "debate" | "agora" | "theory" | "controversy" => Lane::Debate,
        _ => Lane::Fact,
    }
}

pub fn validate_item(
    item: &RawExtractItem,
    document: &str,
    subject: &str,
) -> Result<GroundedItem, RejectReason> {
    if item.quoted_text.trim().is_empty() {
        return Err(RejectReason::EmptyQuote);
    }
    if !quote_is_grounded(document, &item.quoted_text) {
        return Err(RejectReason::QuoteNotInDocument);
    }
    let lane = parse_lane(&item.lane);
    if lane == Lane::Fact && agent_is_other_person(&item.quoted_text, subject) {
        return Err(RejectReason::OtherPersonAgent);
    }
    Ok(GroundedItem {
        lane,
        event_type: if item.event_type.trim().is_empty() {
            "historical_fact".into()
        } else {
            item.event_type.clone()
        },
        role: if item.role.trim().is_empty() {
            "direct".into()
        } else {
            item.role.clone()
        },
        year: item.year,
        place_surface: item.place_surface.clone(),
        summary: item.summary.clone(),
        quoted_text: item.quoted_text.clone(),
        confidence: item.confidence.clamp(0.0, 1.0),
    })
}

pub fn accept_items(
    subject: &str,
    document: &str,
    raw: impl IntoIterator<Item = RawExtractItem>,
) -> Vec<GroundedItem> {
    raw.into_iter()
        .filter_map(|item| validate_item(&item, document, subject).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(lane: &str, quote: &str) -> RawExtractItem {
        RawExtractItem {
            lane: lane.into(),
            event_type: "residence".into(),
            role: "direct".into(),
            year: Some(1920),
            place_surface: Some("Dublin".into()),
            summary: quote.into(),
            quoted_text: quote.into(),
            confidence: 0.8,
        }
    }

    #[test]
    fn quote_must_be_substring() {
        assert!(quote_is_grounded(
            "Born in Warsaw in 1867.",
            "Born in Warsaw in 1867."
        ));
        assert!(!quote_is_grounded("Born in Warsaw.", "Born in Paris."));
    }

    #[test]
    fn drops_other_person_as_agent() {
        let q = "On 6 April 1920, Schrödinger married Annemarie Bertel.";
        assert!(agent_is_other_person(q, "Marie Curie"));
        assert!(!agent_is_other_person(
            "In 1891 Curie moved to Paris.",
            "Marie Curie"
        ));
    }

    #[test]
    fn debate_lane_not_rejected_for_other_agent() {
        let doc = "On 6 April 1920, Schrödinger married Annemarie Bertel.";
        let v = validate_item(&item("debate", doc), doc, "Marie Curie").unwrap();
        assert_eq!(v.lane, Lane::Debate);
    }

    #[test]
    fn schrodinger_quote_is_not_a_curie_fact() {
        let doc = "On 6 April 1920, Schrödinger married Annemarie Bertel. Curie worked in Paris.";
        let raw = vec![item(
            "fact",
            "On 6 April 1920, Schrödinger married Annemarie Bertel.",
        )];
        assert!(accept_items("Marie Curie", doc, raw).is_empty());
    }

    #[test]
    fn statue_quote_about_curie_is_kept() {
        let doc = "A statue of Marie Curie stands in Warsaw.";
        let raw = vec![RawExtractItem {
            lane: "fact".into(),
            event_type: "commemoration".into(),
            role: "indirect".into(),
            year: None,
            place_surface: Some("Warsaw".into()),
            summary: "Statue in Warsaw".into(),
            quoted_text: doc.into(),
            confidence: 0.9,
        }];
        let got = accept_items("Marie Curie", doc, raw);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].event_type, "commemoration");
    }
}
