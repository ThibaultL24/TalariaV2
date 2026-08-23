// crates/talaria-sources/src/place_quality.rs
//! Heuristics to reject non-place surfaces extracted from prose.

const STOP_WORDS: &[&str] = &[
    "the", "a", "an", "his", "her", "their", "its", "this", "that", "these", "those", "which",
    "who", "whom", "whose", "where", "when", "what", "how", "why", "and", "or", "but", "with",
    "from", "into", "onto", "upon", "over", "under", "after", "before", "during", "while",
    "january", "february", "march", "april", "may", "june", "july", "august", "september",
    "october", "november", "december", "spring", "summer", "autumn", "winter", "morning",
    "afternoon", "evening", "night", "year", "years", "month", "months", "day", "days",
    "hiver", "été", "ete", "printemps", "automne",
    "janvier", "février", "fevrier", "mars", "avril", "mai", "juin", "juillet",
    "août", "aout", "septembre", "octobre", "novembre", "décembre", "decembre",
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
    // Must start with a letter, or a numbered street address.
    let first = s.chars().next().unwrap_or(' ');
    if !first.is_alphabetic() && !is_street_address(s) {
        return false;
    }
    // Reject all-lowercase multi-word glue (keep "rue X" / "quai X").
    if s.contains(' ') && s.chars().all(|c| !c.is_uppercase()) && !is_street_address(s) {
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
    if looks_like_non_place(s, &lower) {
        return false;
    }
    true
}

const DEMONYMS: &[&str] = &[
    "anglais", "english", "français", "francais", "french", "allemand", "german",
    "prussien", "prussian", "espagnol", "spanish", "italien", "italian",
    "russe", "russian", "autrichien", "austrian", "américain", "americain", "american",
];

const ABSTRACT_NOUNS: &[&str] = &[
    "schisme", "schism", "concile", "council", "rencontre", "meeting", "encounter",
    "victoire", "victory", "guerre", "war", "paix", "peace", "traité", "traite",
    "treaty", "alliance", "coalition", "révolution", "revolution", "empire",
];

const GIVEN_NAMES: &[&str] = &[
    "charles", "georges", "george", "antoine", "pierre", "jean", "paul", "jacques",
    "henri", "louis", "joseph", "william", "john", "robert", "james", "marie",
    "jeanne", "anne", "catherine", "françois", "francois", "napoleon", "napoléon",
];

const GEO_PREFIXES: &[&str] = &[
    "saint", "st", "st.", "san", "santa", "ste", "sainte", "mount", "mt", "fort",
    "cape", "port", "lake", "île", "ile", "island", "rio", "rue", "quai", "new",
    "los", "las", "el",
];

const TITLE_PREFIXES: &[&str] = &[
    "vicomte", "vicomtesse", "duc", "duchesse", "comte", "comtesse", "marquis",
    "baron", "baronne", "prince", "princesse", "roi", "reine", "king", "queen",
    "emperor", "empress", "empereur", "impératrice",
];

fn looks_like_non_place(original: &str, lower: &str) -> bool {
    if DEMONYMS.iter().any(|d| lower == *d) {
        return true;
    }
    if ABSTRACT_NOUNS.iter().any(|n| lower == *n) {
        return true;
    }
    if lower.starts_with("la rencontre")
        || lower.starts_with("le concile")
        || lower.starts_with("the meeting")
        || lower.starts_with("the encounter")
        || lower.starts_with("the council")
    {
        return true;
    }
    if lower.contains(" et ") || lower.contains(" and ") {
        return true;
    }
    let tokens: Vec<&str> = lower.split_whitespace().collect();
    if tokens.is_empty() {
        return false;
    }
    if TITLE_PREFIXES.iter().any(|t| tokens[0] == *t || tokens[0].starts_with(&format!("{t}-"))) {
        return true;
    }
    if tokens.iter().any(|tok| GEO_PREFIXES.contains(tok)) || is_street_address(original) {
        return false;
    }
    // "Lyon Charles", "Georges-Antoine Rochegrosse"
    let last = tokens[tokens.len() - 1].trim_matches(|c: char| !c.is_alphabetic());
    if tokens.len() >= 2 && GIVEN_NAMES.contains(&last) {
        return true;
    }
    if tokens.len() >= 2 {
        let first = tokens[0];
        let first_parts: Vec<&str> = first.split('-').collect();
        if first_parts.len() >= 2 && first_parts.iter().all(|p| GIVEN_NAMES.contains(p)) {
            return true;
        }
        if GIVEN_NAMES.contains(&first) && tokens.len() <= 3 {
            return true;
        }
    }
    false
}

fn is_street_address(s: &str) -> bool {
    let lower = s.to_lowercase();
    let body = lower.trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ' || c == ',');
    body.starts_with("rue ")
        || body.starts_with("quai ")
        || body.starts_with("place ")
        || body.starts_with("square ")
        || body.starts_with("avenue ")
        || body.starts_with("boulevard ")
        || body.starts_with("hôtel ")
        || body.starts_with("hotel ")
        || body.starts_with("impasse ")
        || body.starts_with("chemin ")
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

    #[test]
    fn accepts_street_and_building_addresses() {
        assert!(is_plausible_place_label("rue Meslay"));
        assert!(is_plausible_place_label("19 quai Malaquais"));
        assert!(is_plausible_place_label("hôtel Danieli"));
        assert!(is_plausible_place_label("square d'Orléans"));
    }

    #[test]
    fn rejects_people_demonyms_and_abstractions() {
        assert!(!is_plausible_place_label("Lyon Charles"));
        assert!(!is_plausible_place_label("Georges-Antoine Rochegrosse"));
        assert!(!is_plausible_place_label("Anglais"));
        assert!(!is_plausible_place_label("schisme"));
        assert!(!is_plausible_place_label("la rencontre de Nobel et Hess"));
        assert!(is_plausible_place_label("Waterloo"));
        assert!(is_plausible_place_label("Saint Helena"));
        assert!(is_plausible_place_label("New York"));
    }
}
