// crates/talaria-quality/src/documentary.rs
//! What a stored passage is allowed to do for an event.
//!
//! A catalog title can be discovered. It does not attest the event unless the
//! quote itself narrates that event.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentaryRelation {
    /// The quote narrates this event.
    Direct,
    /// The document is about the same place or episode, without a passage.
    Context,
    /// A bibliographic notice: title, catalog record, or citation line.
    BibliographicNotice,
    /// The notice is about something else.
    OffTopic,
    /// Not enough to decide. Stays in review when a gate asks.
    Undetermined,
}

const CATALOG_TAGS: &[&str] = &[
    "[hal]",
    "[open_alex]",
    "[openalex]",
    "[gallica]",
    "[persee]",
    "[theses",
    "[bnf]",
    "[wikidata]",
];

/// A quote that is only a catalog record, not a narrative passage.
pub fn is_bibliographic_quote(quote: &str) -> bool {
    let lower = quote.trim().to_lowercase();
    if lower.is_empty() {
        return false;
    }
    CATALOG_TAGS.iter().any(|tag| lower.starts_with(tag))
        || lower.contains("isbn")
        || lower.contains("hal.science/")
            && !lower.contains(" was fought")
            && !lower.contains(" fut ")
}

/// Classify a quote against one event focus, usually the place name.
pub fn documentary_relation(quote: &str, event_focus: Option<&str>) -> DocumentaryRelation {
    let focus = event_focus
        .map(str::trim)
        .filter(|focus| focus.chars().count() >= 4);
    if is_bibliographic_quote(quote) {
        return match focus {
            Some(focus) if !contains_focus(quote, focus) => DocumentaryRelation::OffTopic,
            _ => DocumentaryRelation::BibliographicNotice,
        };
    }
    if is_direct_passage(quote, focus) {
        return DocumentaryRelation::Direct;
    }
    if looks_like_unattached_title(quote) {
        return match focus {
            Some(focus) if contains_focus(quote, focus) => DocumentaryRelation::Context,
            Some(_) => DocumentaryRelation::OffTopic,
            None => DocumentaryRelation::BibliographicNotice,
        };
    }
    DocumentaryRelation::Undetermined
}

/// A bibliographic notice must not be inserted as attesting evidence.
pub fn quote_may_attest(quote: &str, event_focus: Option<&str>) -> bool {
    matches!(
        documentary_relation(quote, event_focus),
        DocumentaryRelation::Direct | DocumentaryRelation::Undetermined
    )
}

fn is_direct_passage(quote: &str, focus: Option<&str>) -> bool {
    let lower = quote.to_lowercase();
    let narrates = [
        " was fought",
        " was signed",
        " signed on ",
        " signed at ",
        "truce signed",
        "peace treaty",
        " were fought",
        "fut livr",
        "fut combatt",
        "bataille de ",
        "bataille d'",
        "battle of ",
        "siege of ",
        "siège de ",
        " was born",
        " died ",
        " meurt",
        " naît",
        " nait ",
        "publia",
        "published",
        "découvrit",
        "discovered",
        "demeure au",
        "demeure à",
        "vécut",
        "habitait",
        "séjourna",
        "résida",
        "lived in",
        "rencontra",
        " met ",
        "est élu",
        "fut élu",
        "est nommé",
    ]
    .iter()
    .any(|cue| lower.contains(cue));
    narrates && focus.is_none_or(|focus| contains_focus(quote, focus))
}

fn looks_like_unattached_title(quote: &str) -> bool {
    let trimmed = quote.trim();
    trimmed.chars().count() < 220 && !trimmed.contains(". ") && !is_direct_passage(trimmed, None)
}

fn contains_focus(quote: &str, focus: &str) -> bool {
    let quote_words: Vec<String> = focus_words(quote);
    let focus_words = focus_words(focus);
    if focus_words.is_empty() {
        return false;
    }
    quote_words
        .windows(focus_words.len())
        .any(|window| window == focus_words.as_slice())
}

fn focus_words(value: &str) -> Vec<String> {
    fold(value)
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '\'' || ch == '-' {
                ch
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

pub(crate) fn fold(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'à' | 'á' | 'â' | 'ä' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NarrativeReading {
    /// A singular episode with a passage. Not yet an editorial validation.
    Candidate,
    /// The text itself says the story is legendary or unfounded.
    Apocryphal,
    /// A major typed event, not an anecdote.
    MajorEvent,
    NotAnAnecdote,
}

/// Editorial interest. This does not change the canonical type and does not
/// validate the episode.
pub fn narrative_reading(event_type: &str, title: &str, quote: &str) -> NarrativeReading {
    let text = fold(&format!("{title} {quote}"));
    if [
        "apocryph",
        "unfounded",
        "legende",
        "legend",
        "mythe",
        "myth",
        "on dit que",
    ]
    .iter()
    .any(|cue| text.contains(cue))
    {
        return NarrativeReading::Apocryphal;
    }
    if matches!(
        event_type,
        "battle" | "siege" | "birth" | "death" | "military_campaign"
    ) {
        return NarrativeReading::MajorEvent;
    }
    if is_bibliographic_quote(quote) || is_bibliographic_quote(title) {
        return NarrativeReading::NotAnAnecdote;
    }
    let singular = [
        " a letter",
        " his letter",
        " une lettre",
        "newspaper",
        " in a cafe",
        "yellow fever",
        "fievre jaune",
    ]
    .iter()
    .any(|cue| text.contains(cue));
    if singular {
        return NarrativeReading::Candidate;
    }
    if matches!(
        event_type,
        "meeting" | "residence" | "travel" | "exile" | "marriage" | "anecdote"
    ) && text.chars().count() > 40
    {
        return NarrativeReading::Candidate;
    }
    NarrativeReading::NotAnAnecdote
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnecdoteStanding {
    NotAnAnecdote,
    Apocryphal,
    /// One passage, not yet checked against another source.
    Candidate,
    /// A precise passage with a date. Still not an editorial approval.
    Documented,
    /// Set only when a review explicitly accepts the passage.
    Approved,
}

/// Wikipedia alone never produces Approved. `reviewed` must be an explicit decision.
pub fn anecdote_standing(event_type: &str, quote: &str, reviewed: bool) -> AnecdoteStanding {
    match narrative_reading(event_type, quote, quote) {
        NarrativeReading::Apocryphal => AnecdoteStanding::Apocryphal,
        NarrativeReading::NotAnAnecdote | NarrativeReading::MajorEvent => {
            AnecdoteStanding::NotAnAnecdote
        }
        NarrativeReading::Candidate => {
            let lower = fold(quote);
            let hedged = [
                "would have",
                "usually identified",
                "alleged",
                "perhaps",
                "on dit",
                "peut-etre",
                "peut-être",
            ]
            .iter()
            .any(|cue| lower.contains(cue));
            let dated = quote.chars().filter(|ch| ch.is_ascii_digit()).count() >= 4
                && quote.split(|ch: char| !ch.is_ascii_digit()).any(|word| word.len() == 4);
            if reviewed && dated && !hedged {
                AnecdoteStanding::Approved
            } else if dated && !hedged {
                AnecdoteStanding::Documented
            } else {
                AnecdoteStanding::Candidate
            }
        }
    }
}

/// Paragraphs that mention a missing fact, including one earlier paragraph when it carries a date.
pub fn relevant_paragraphs<'a>(text: &'a str, needles: &[&str]) -> Vec<&'a str> {
    let needles: Vec<String> = needles.iter().map(|needle| fold(needle)).collect();
    let paras: Vec<&str> = text
        .split("\n\n")
        .map(str::trim)
        .filter(|para| para.chars().count() > 30)
        .collect();
    let mut chosen = Vec::new();
    for (index, para) in paras.iter().enumerate() {
        let folded = fold(para);
        if !needles.iter().any(|needle| folded.contains(needle)) {
            continue;
        }
        if index > 0
            && para.chars().count() < 120
            && !para_has_year(para)
            && paras[index - 1].chars().count() < 240
            && para_has_year(paras[index - 1])
        {
            let previous = paras[index - 1];
            if !chosen.contains(&previous) {
                chosen.push(previous);
            }
        }
        if !chosen.contains(para) {
            chosen.push(*para);
        }
        if chosen.len() >= 4 {
            break;
        }
    }
    chosen
}

fn para_has_year(para: &str) -> bool {
    para.split(|ch: char| !ch.is_ascii_digit())
        .any(|word| word.len() == 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn italian_campaign_catalogs_do_not_attest_the_battle() {
        let battles = [
            (
                "Lonato",
                "[HAL] Napoléon et Milan : mise en scène, réception et délégation du pouvoir napoléonien",
            ),
            (
                "Millesimo",
                "[OPEN_ALEX] Russia and the Napoleonic Wars, 1807-1815",
            ),
            (
                "Mondovì",
                "[GALLICA] Les Adieux de Napoléon Bonaparte, à l'impératrice Marie-Louise",
            ),
            (
                "Rivoli",
                "[HAL] The Emperor and his Entourage. Napoleonic Portraiture",
            ),
        ];
        for (place, notice) in battles {
            assert_eq!(
                documentary_relation(notice, Some(place)),
                DocumentaryRelation::OffTopic,
                "{place}"
            );
            assert!(!quote_may_attest(notice, Some(place)), "{place}");
        }
    }

    #[test]
    fn a_catalog_title_that_names_rivoli_is_context_not_proof() {
        let notice = "[HAL] Bibliographie du combat de Rivoli";
        assert_eq!(
            documentary_relation(notice, Some("Rivoli")),
            DocumentaryRelation::BibliographicNotice
        );
        assert!(!quote_may_attest(notice, Some("Rivoli")));
    }

    #[test]
    fn an_untagged_bibliography_title_is_context_not_proof() {
        let title = "Bibliographie du combat de Rivoli";
        assert_eq!(
            documentary_relation(title, Some("Rivoli")),
            DocumentaryRelation::Context
        );
        assert!(!quote_may_attest(title, Some("Rivoli")));
    }

    #[test]
    fn a_residence_and_a_publication_remain_direct_passages() {
        let residence =
            "À partir de 1682 la cour demeure au château de Versailles pour tout le règne.";
        assert_eq!(
            documentary_relation(residence, Some("Versailles")),
            DocumentaryRelation::Direct
        );
        let book = "En 1862 Victor Hugo publia Les Misérables à Paris.";
        assert!(quote_may_attest(book, Some("Paris")));
    }

    #[test]
    fn the_wikipedia_sentence_attests_lonato() {
        let quote = "The Battle of Lonato was fought on 3 and 4 August 1796 between the French Army of Italy and an Austrian corps.";
        assert_eq!(
            documentary_relation(quote, Some("Lonato")),
            DocumentaryRelation::Direct
        );
        assert!(quote_may_attest(quote, Some("Lonato")));
    }

    #[test]
    fn an_apocryphal_hugo_letter_is_not_an_established_anecdote() {
        let title = "An apocryphal tale has circulated, describing the shortest correspondence in history";
        assert_eq!(
            narrative_reading("anecdote", title, title),
            NarrativeReading::Apocryphal
        );
    }

    #[test]
    fn a_battle_is_not_reclassified_as_an_anecdote() {
        assert_eq!(
            narrative_reading("battle", "Battle of Rivoli", "fought on 14 January 1797"),
            NarrativeReading::MajorEvent
        );
    }

    #[test]
    fn louvre_rivoli_is_not_the_battle_of_rivoli() {
        let title = "Bombage de la station Louvre-Rivoli le 1er mai 1991";
        assert_eq!(
            documentary_relation(title, Some("Rivoli")),
            DocumentaryRelation::OffTopic
        );
    }

    #[test]
    fn a_scientific_paper_is_not_an_anecdote() {
        let clause = "In July 1898, Curie and her husband published a joint paper announcing the existence of an element they named polonium.";
        assert_eq!(
            narrative_reading("publication", clause, clause),
            NarrativeReading::NotAnAnecdote
        );
    }

    #[test]
    fn a_newspaper_in_a_cafe_is_only_a_candidate_anecdote() {
        let clause = "Hugo was travelling in 1843 when he read the news in a newspaper in a cafe.";
        assert_eq!(
            narrative_reading("travel", clause, clause),
            NarrativeReading::Candidate
        );
    }

    #[test]
    fn a_dated_letter_is_documented_and_not_approved() {
        let clause = "In 1859, he wrote a letter asking the United States to spare John Brown.";
        assert_eq!(
            anecdote_standing("meeting", clause, false),
            AnecdoteStanding::Documented
        );
        assert_eq!(
            anecdote_standing("meeting", clause, true),
            AnecdoteStanding::Approved
        );
    }

    #[test]
    fn a_hedged_identification_stays_a_candidate() {
        let clause = "Caterina is usually identified as the Caterina Buti del Vacca, who married in 1452.";
        assert_eq!(
            anecdote_standing("meeting", clause, false),
            AnecdoteStanding::Candidate
        );
    }

    #[test]
    fn an_exile_paragraph_beyond_the_introduction_is_selected() {
        let filler = "Victor Hugo published poems in Paris.\n\n".repeat(40);
        let text = format!("{filler}From 1855 Victor Hugo lived in exile on Guernsey.\n\n");
        let found = relevant_paragraphs(&text, &["guernsey", "exile"]);
        assert!(found.iter().any(|para| para.contains("Guernsey")), "{found:?}");
        assert!(found.len() < 5, "{found:?}");
    }
}
