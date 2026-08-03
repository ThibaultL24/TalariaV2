// crates/talaria-sources/src/place_quality.rs
//! Heuristics to reject non-place surfaces extracted from prose.

const STOP_WORDS: &[&str] = &[
    "the", "a", "an", "his", "her", "their", "its", "this", "that", "these", "those", "which",
    "who", "whom", "whose", "where", "when", "what", "how", "why", "and", "or", "but", "with",
    "from", "into", "onto", "upon", "over", "under", "after", "before", "during", "while",
    "january", "february", "march", "april", "may", "june", "july", "august", "september",
    "october", "november", "december", "spring", "summer", "autumn", "winter", "morning",
    "afternoon", "evening", "night", "year", "years", "month", "months", "day", "days",
    "week", "weeks", "time", "times", "order", "command", "battle", "war", "campaign",
    "siege", "army", "armies", "force", "forces", "troops", "victory", "defeat", "retreat",
];

const MONTHS: &[&str] = &[
    "january", "february", "march", "april", "may", "june", "july", "august", "september",
    "october", "november", "december",
];

/// True when a surface looks like a geographic place label (not a clause fragment).
pub fn is_plausible_place_label(raw: &str) -> bool {
    let s = raw.trim();
    if s.len() < 2 || s.len() > 60 {
        return false;
    }
    if s.contains('\n') || s.matches(' ').count() > 5 {
        return false;
    }
    // Reject sentence-like fragments
    let lower = s.to_lowercase();
    if lower.contains(" the ")
        || lower.starts_with("the ")
        || lower.starts_with("a ")
        || lower.starts_with("an ")
        || lower.starts_with("his ")
        || lower.starts_with("her ")
        || lower.starts_with("their ")
        || lower.starts_with("have ")
        || lower.starts_with("fight")
        || lower.starts_with("chapter")
        || lower.contains("napoleon")
        || lower.contains("coalition")
        || lower.contains("claim")
    {
        return false;
    }
    if MONTHS.iter().any(|m| lower == *m) {
        return false;
    }
    if STOP_WORDS.iter().any(|w| lower == *w) {
        return false;
    }
    // Must start with a letter; prefer capitalised proper nouns
    let first = s.chars().next().unwrap_or(' ');
    if !first.is_alphabetic() {
        return false;
    }
    // Reject all-lowercase multi-word glue
    if s.contains(' ') && s.chars().all(|c| !c.is_uppercase()) {
        return false;
    }
    // Must not end mid-phrase with prepositions/articles
    if lower.ends_with(" on")
        || lower.ends_with(" in")
        || lower.ends_with(" at")
        || lower.ends_with(" of")
        || lower.ends_with(" the")
        || lower.ends_with('(')
        || lower.ends_with('-')
    {
        return false;
    }
    // Digits-only or year-like
    if s.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_waterloo() {
        assert!(is_plausible_place_label("Waterloo"));
        assert!(is_plausible_place_label("Saint Helena"));
        assert!(is_plausible_place_label("Jena–Auerstedt"));
    }

    #[test]
    fn rejects_noise() {
        assert!(!is_plausible_place_label("November"));
        assert!(!is_plausible_place_label("his youth"));
        assert!(!is_plausible_place_label("the afternoon"));
        assert!(!is_plausible_place_label("have regained the initiative"));
        assert!(!is_plausible_place_label("fight"));
        assert!(!is_plausible_place_label("Portoferraio on"));
        assert!(!is_plausible_place_label("Abukir ("));
    }
}
