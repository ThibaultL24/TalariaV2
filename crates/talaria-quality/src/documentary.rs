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
    fold(quote).contains(&fold(focus))
}

fn fold(value: &str) -> String {
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
    if matches!(
        event_type,
        "meeting" | "residence" | "travel" | "exile" | "marriage" | "publication" | "anecdote"
    ) && text.chars().count() > 40
    {
        return NarrativeReading::Candidate;
    }
    NarrativeReading::NotAnAnecdote
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
}
