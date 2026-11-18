// crates/talaria-quality/src/public_admission.rs
//! What may leave review and enter the public biography.
//!
//! A technical accept is not enough. The passage has to show who acts, and a
//! catalog line, a relative's action, or a borrowed year stays in review.

/// Publish the fact, or keep the candidate in review with a stable reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicAdmission {
    Publish,
    Hold(&'static str),
}

/// One extracted fact, judged before it can become a canonical event.
pub struct PublicFact<'a> {
    pub subject: &'a str,
    pub quote: &'a str,
    pub summary: &'a str,
    pub event_type: &'a str,
    pub time_reading: &'a str,
    pub structured_source: bool,
}

const KIN_CUES: &[&str] = &[
    "sa fille",
    "son fils",
    "sa soeur",
    "sa sœur",
    "son frere",
    "son frère",
    "sa femme",
    "son mari",
    "son epoux",
    "son époux",
    "his daughter",
    "her daughter",
    "his son",
    "her son",
    "his wife",
    "her husband",
    "his sister",
    "her sister",
    "his brother",
    "her brother",
];

const ACTION_VERBS: &[&str] = &[
    "compose",
    "composed",
    "composa",
    "écrit",
    "ecrit",
    "wrote",
    "written",
    "published",
    "publia",
    "publie",
    "fought",
    "signed",
    "signa",
    "commanded",
    "drowned",
    "painted",
    "peignit",
    "sculpta",
];

const OPENERS: &[&str] = &[
    "the", "a", "an", "le", "la", "les", "un", "une", "en", "on", "in", "at", "de", "du",
    "des", "sa", "son", "his", "her", "their", "after", "après", "apres", "battle", "siege",
    "treaty", "armistice", "second", "general", "king",
];

const NOTICE_MARKERS: &[&str] = &[
    "gallica",
    "imprimerie",
    "impr.",
    "isbn",
    "ouvrage collectif",
    "deuxième édition",
    "deuxieme edition",
    "seconde édition",
    "seconde edition",
    "réédition",
    "reedition",
    "reprint",
    "catalogue",
    "catalog record",
    "notice bibliographique",
    "publiées par",
    "publiees par",
    "published by",
];

const LOCALITIES: &[&str] = &[
    "paris", "lyon", "douai", "rouen", "bruxelles", "brussels", "genève", "geneve", "geneva",
    "guernsey", "guernesey", "ajaccio", "marseille", "rome", "london", "londres", "waterloo",
    "mantua", "mantoue", "cherasco", "arcole", "castiglione",
];

const TERRITORIES: &[&str] = &[
    "france", "england", "angleterre", "austria", "autriche", "russia", "russie", "spain",
    "espagne", "italy", "italie", "germany", "allemagne", "poland", "pologne", "belgium",
    "belgique", "europe", "prussia", "prusse", "royaume de france",
];

pub fn admit_to_public_corpus(fact: &PublicFact<'_>) -> PublicAdmission {
    if fact.time_reading == "inferred_year" || fact.time_reading == "paragraph_context" {
        return PublicAdmission::Hold("date_not_in_the_passage");
    }
    if is_documentary_notice(&fact.quote) || is_documentary_notice(&fact.summary) {
        return PublicAdmission::Hold("bibliographic_notice");
    }
    let passage = format!("{} {}", fact.summary, fact.quote);
    if someone_else_acts(&passage, fact.subject) {
        return PublicAdmission::Hold("actor_is_not_the_subject");
    }
    if is_parentage_only(&fact.quote) {
        return PublicAdmission::Hold("genealogical_context");
    }
    if !fact.structured_source && fact.quote.split_whitespace().count() < 4 {
        return PublicAdmission::Hold("passage_too_thin");
    }
    if !fact.structured_source && !passage_has_an_actor(&fact.quote, fact.subject) {
        return PublicAdmission::Hold("actor_not_in_the_passage");
    }
    let _ = fact.event_type;
    PublicAdmission::Publish
}

/// Two cities in one sentence are not one map point.
pub fn locality_count(quote: &str) -> usize {
    let folded = fold(quote);
    LOCALITIES
        .iter()
        .filter(|name| contains_word(&folded, name))
        .count()
}

pub fn quote_names_several_localities(quote: &str) -> bool {
    locality_count(quote) >= 2
}

/// A country or kingdom name is not a precise place for a map point.
pub fn is_political_territory(label: &str) -> bool {
    let folded = fold(label.trim());
    TERRITORIES.iter().any(|name| folded == *name)
}

/// Letters and digits only, so a final period does not create a second fact.
pub fn clause_identity(quote: &str) -> String {
    quote
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn someone_else_acts(text: &str, subject: &str) -> bool {
    let folded = fold(text);
    if KIN_CUES.iter().any(|cue| folded.contains(cue))
        && ACTION_VERBS.iter().any(|verb| folded.contains(verb))
    {
        return true;
    }
    if notice_names_another_maker(&folded, subject) || acted_under_someone_else(&folded, subject) {
        return true;
    }
    leading_name_acts(text, subject)
}

fn acted_under_someone_else(folded: &str, subject: &str) -> bool {
    let (given, surname) = subject_names(subject);
    for marker in ["fought by ", "commanded by ", "under general ", "signed by "] {
        let Some(index) = folded.find(marker) else {
            continue;
        };
        let after = &folded[index + marker.len()..];
        let window = after.split_whitespace().take(8).collect::<Vec<_>>().join(" ");
        let subject_acts = contains_word(&window, &surname)
            || (given.chars().count() >= 4 && contains_word(&window, &given));
        if !subject_acts {
            return true;
        }
    }
    false
}

fn notice_names_another_maker(folded: &str, subject: &str) -> bool {
    let (_, surname) = subject_names(subject);
    if surname.is_empty() {
        return false;
    }
    for marker in [
        "publiées par ",
        "publiees par ",
        "published by ",
        "composée par ",
        "composee par ",
        "composées par ",
        "composees par ",
        "composé par ",
        "composed by ",
    ] {
        if let Some(index) = folded.find(marker) {
            let after = folded[index + marker.len()..].trim();
            let maker = after.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
            if !maker.is_empty() && !maker.contains(&surname) {
                return true;
            }
        }
    }
    false
}

fn leading_name_acts(text: &str, subject: &str) -> bool {
    let (given, surname) = subject_names(subject);
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut index = 0;
    while index < words.len() {
        let token = clean_token(words[index]);
        if token.is_empty() || OPENERS.contains(&token.as_str()) || token.chars().all(|ch| ch.is_ascii_digit())
        {
            index += 1;
            continue;
        }
        break;
    }
    let Some(first) = words.get(index) else {
        return false;
    };
    if !first.chars().next().is_some_and(|ch| ch.is_uppercase()) {
        return false;
    }
    let first_token = clean_token(first);
    if token_is_subject(&first_token, &given, &surname) {
        return false;
    }
    let second_raw = words.get(index + 1).copied().unwrap_or("");
    let third_raw = words.get(index + 2).copied().unwrap_or("");
    let second = clean_token(second_raw);
    let third = clean_token(third_raw);
    if token_is_subject(&second, &given, &surname) {
        return true;
    }
    if ACTION_VERBS.iter().any(|verb| second.starts_with(verb)) {
        return true;
    }
    if second_raw.chars().next().is_some_and(|ch| ch.is_uppercase())
        && ACTION_VERBS.iter().any(|verb| third.starts_with(verb))
    {
        return true;
    }
    let folded = fold(text);
    (folded.contains("monument") || folded.contains("statue"))
        && first.chars().next().is_some_and(|ch| ch.is_uppercase())
        && second_raw.chars().next().is_some_and(|ch| ch.is_uppercase())
}

fn passage_has_an_actor(quote: &str, subject: &str) -> bool {
    let folded = fold(quote);
    let (given, surname) = subject_names(subject);
    if !surname.is_empty() && contains_word(&folded, &surname) {
        return true;
    }
    if given.chars().count() >= 4 && contains_word(&folded, &given) {
        return true;
    }
    const CORE: &[&str] = &[
        "elle", "il", "she", "he", "they", "couple", "jeune", "young",
    ];
    CORE.iter().any(|word| contains_word(&folded, word))
}

fn is_documentary_notice(text: &str) -> bool {
    let folded = fold(text);
    NOTICE_MARKERS.iter().any(|marker| folded.contains(marker))
}

fn is_parentage_only(quote: &str) -> bool {
    let folded = fold(quote);
    let parentage = folded.contains("fille de")
        || folded.contains("fils de")
        || folded.contains("daughter of")
        || folded.contains("son of");
    parentage && !ACTION_VERBS.iter().any(|verb| folded.contains(verb))
}

fn subject_names(subject: &str) -> (String, String) {
    let given = subject.split_whitespace().next().unwrap_or("").to_string();
    let surname = subject.split_whitespace().last().unwrap_or("").to_string();
    (fold(&given), fold(&surname))
}

fn token_is_subject(token: &str, given: &str, surname: &str) -> bool {
    let token = fold(token);
    !token.is_empty() && (token == given || token == surname)
}

fn contains_word(folded: &str, word: &str) -> bool {
    folded
        .split(|ch: char| !ch.is_alphabetic() && ch != '-' && ch != '\'')
        .any(|token| token == word)
}

fn clean_token(word: &str) -> String {
    fold(word.trim_matches(|ch: char| !ch.is_alphabetic() && ch != '-' && ch != '\''))
}

fn fold(text: &str) -> String {
    text.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact<'a>(
        subject: &'a str,
        quote: &'a str,
        reading: &'a str,
    ) -> PublicFact<'a> {
        PublicFact {
            subject,
            quote,
            summary: quote,
            event_type: "work",
            time_reading: reading,
            structured_source: false,
        }
    }

    #[test]
    fn a_daughters_poems_are_not_the_subjects_publication() {
        let full = "Sa fille Hyacinthe compose des poèmes et des contes.";
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Marceline Desbordes-Valmore",
                full,
                "missing"
            )),
            PublicAdmission::Hold("actor_is_not_the_subject")
        );
        let short = PublicFact {
            subject: "Marceline Desbordes-Valmore",
            quote: "compose des poèmes et des contes",
            summary: full,
            event_type: "work",
            time_reading: "missing",
            structured_source: false,
        };
        assert_eq!(
            admit_to_public_corpus(&short),
            PublicAdmission::Hold("actor_is_not_the_subject")
        );
    }

    #[test]
    fn a_borrowed_year_is_not_a_public_date() {
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Marceline Desbordes-Valmore",
                "the young girl somehow made her way back to France",
                "inferred_year"
            )),
            PublicAdmission::Hold("date_not_in_the_passage")
        );
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Marceline Desbordes-Valmore",
                "the young girl somehow made her way back to France",
                "missing"
            )),
            PublicAdmission::Publish
        );
        assert!(is_political_territory("France"));
        assert!(!is_political_territory("Douai"));
    }

    #[test]
    fn a_gallica_edition_is_a_notice_not_a_life_event() {
        let quote = "deuxième édition (Poésies de Mme Desbordes-Valmore publiées par Gustave Revilliod) sur Gallica, Genève, Jules Fick, 1873";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", quote, "explicit_year")),
            PublicAdmission::Hold("bibliographic_notice")
        );
    }

    #[test]
    fn a_later_reprint_and_a_statue_are_not_her_actions() {
        let statue = "Édouard Houssin, Monument à Marceline Desbordes-Valmore, 1896, statue en bronze, Douai.";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", statue, "explicit_year")),
            PublicAdmission::Hold("actor_is_not_the_subject")
        );
        let book = "Auguste Bleton, Marceline Desbordes-Valmore à Lyon, Lyon, Impr. de A. Rey, 1896";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", book, "explicit_year")),
            PublicAdmission::Hold("bibliographic_notice")
        );
    }

    #[test]
    fn the_same_lyon_sentence_is_one_clause() {
        assert_eq!(
            clause_identity("Le couple s'installe ensuite à Lyon"),
            clause_identity("Le couple s'installe ensuite à Lyon.")
        );
    }

    #[test]
    fn a_dated_publication_in_the_sentence_can_be_public() {
        let quote = "En 1820 paraissent les Poésies de Mme Desbordes-Valmore.";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", quote, "explicit_year")),
            PublicAdmission::Publish
        );
    }

    #[test]
    fn another_composers_settings_stay_in_review() {
        let louise = "parmi les dix romances composées par Louise de Caumont Dadde on trouve cinq mises en musique de poèmes de Marceline Desbordes-Valmore";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", louise, "missing")),
            PublicAdmission::Hold("actor_is_not_the_subject")
        );
        let woollett = "Henri Woollett compose la suite Marceline ou la Vie d'une femme";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", woollett, "explicit_year")),
            PublicAdmission::Hold("actor_is_not_the_subject")
        );
    }

    #[test]
    fn parentage_and_a_fragment_stay_in_review() {
        let parents = "Marceline Desbordes est la fille de Catherine Lucas et de Félix Desbordes, un peintre en armoiries";
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", parents, "missing")),
            PublicAdmission::Hold("genealogical_context")
        );
        assert_eq!(
            admit_to_public_corpus(&fact("Marceline Desbordes-Valmore", "et à Paris", "missing")),
            PublicAdmission::Hold("passage_too_thin")
        );
    }

    #[test]
    fn one_career_sentence_is_not_several_cities() {
        let quote = "Desbordes-Valmore appeared as an actress and singer in Douai, Rouen, the Opéra-Comique in Paris, and the Théâtre de la Monnaie in Brussels";
        assert!(quote_names_several_localities(quote));
        assert!(!quote_names_several_localities(
            "Le couple s'installe ensuite à Lyon"
        ));
    }

    #[test]
    fn napoleon_keeps_a_command_and_loses_another_generals_battle() {
        let arcole = "The Battle of Arcole (15–17 November 1796) was fought southeast of Verona. The battle saw a bold maneuver by Napoleon Bonaparte's French Army of Italy.";
        assert_eq!(
            admit_to_public_corpus(&fact("Napoleon", arcole, "explicit_day")),
            PublicAdmission::Publish
        );
        let ceva = "The Battle of Ceva was fought by the French under General Pierre Augereau.";
        assert_eq!(
            admit_to_public_corpus(&fact("Napoleon", ceva, "explicit_day")),
            PublicAdmission::Hold("actor_is_not_the_subject")
        );
        let murat = "The Battle of Tolentino was fought by the Napoleonic King of Naples Joachim Murat.";
        assert_eq!(
            admit_to_public_corpus(&fact("Napoleon", murat, "missing")),
            PublicAdmission::Hold("actor_is_not_the_subject")
        );
    }

    #[test]
    fn hugo_and_curie_keep_their_own_work_and_lose_family_and_reprints() {
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Victor Hugo",
                "In 1855 Victor Hugo lived in Guernsey.",
                "explicit_year"
            )),
            PublicAdmission::Publish
        );
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Victor Hugo",
                "Léopoldine Hugo drowned in the Seine in 1843.",
                "explicit_year"
            )),
            PublicAdmission::Hold("actor_is_not_the_subject")
        );
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Victor Hugo",
                "Victor Hugo published Les Misérables in 1862.",
                "explicit_year"
            )),
            PublicAdmission::Publish
        );
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Victor Hugo",
                "Édition illustrée des Misérables, Paris, Hetzel, sur Gallica.",
                "explicit_year"
            )),
            PublicAdmission::Hold("bibliographic_notice")
        );
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Marie Curie",
                "Marie Curie and Pierre Curie announced the discovery of polonium in 1898.",
                "explicit_year"
            )),
            PublicAdmission::Publish
        );
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Marie Curie",
                "A reprint of Marie Curie's doctoral thesis was issued in Paris in 1903.",
                "explicit_year"
            )),
            PublicAdmission::Hold("bibliographic_notice")
        );
    }

    #[test]
    fn an_ancient_birth_sentence_is_not_a_notice() {
        assert_eq!(
            admit_to_public_corpus(&fact(
                "Aristotle",
                "Aristotle was born in Stagira in 384 BC.",
                "explicit_year"
            )),
            PublicAdmission::Publish
        );
    }
}
