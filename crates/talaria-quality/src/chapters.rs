// crates/talaria-quality/src/chapters.rs
//! Deterministic chapter suggestions from documented event types.
//! A suggestion is not a draft and not a published chapter.

pub const MIN_CHAPTER_EVENTS: usize = 6;
pub const SUGGESTION: &str = "suggestion";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChapterSuggestion {
    pub slug: &'static str,
    pub title_fr: &'static str,
    pub title_en: &'static str,
    pub event_count: usize,
    pub standing: &'static str,
}

struct Theme {
    slug: &'static str,
    title_fr: &'static str,
    title_en: &'static str,
    types: &'static [&'static str],
}

const THEMES: &[Theme] = &[
    Theme { slug: "formation", title_fr: "Formation", title_en: "Education", types: &["education"] },
    Theme { slug: "works", title_fr: "Œuvres", title_en: "Works", types: &["publication", "work", "creation"] },
    Theme { slug: "discoveries", title_fr: "Découvertes", title_en: "Discoveries", types: &["discovery", "invention"] },
    Theme { slug: "research", title_fr: "Recherches", title_en: "Research", types: &["research", "study"] },
    Theme { slug: "campaigns", title_fr: "Campagnes", title_en: "Campaigns", types: &["battle", "siege", "military_campaign"] },
    Theme { slug: "travel", title_fr: "Voyages", title_en: "Journeys", types: &["travel", "visit"] },
    Theme { slug: "relations", title_fr: "Relations", title_en: "Relations", types: &["marriage", "meeting"] },
    Theme { slug: "politics", title_fr: "Engagements politiques", title_en: "Political life", types: &["office", "diplomatic", "treaty"] },
    Theme { slug: "exile", title_fr: "Exils", title_en: "Exiles", types: &["exile"] },
    Theme { slug: "residences", title_fr: "Résidences", title_en: "Residences", types: &["residence", "lived"] },
];

/// Group only the types that are actually documented. A thin corpus stays without a chapter.
pub fn suggest_chapters(counts: &[(&str, usize)]) -> Vec<ChapterSuggestion> {
    let mut suggestions = Vec::new();
    for theme in THEMES {
        let event_count = theme.types.iter().map(|kind| count_of(counts, kind)).sum();
        if event_count < MIN_CHAPTER_EVENTS {
            continue;
        }
        suggestions.push(ChapterSuggestion {
            slug: theme.slug,
            title_fr: theme.title_fr,
            title_en: theme.title_en,
            event_count,
            standing: SUGGESTION,
        });
    }
    suggestions
}

fn count_of(counts: &[(&str, usize)], kind: &str) -> usize {
    counts
        .iter()
        .find(|(event_type, _)| *event_type == kind)
        .map(|(_, count)| *count)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_campaign_corpus_suggests_campaigns_and_not_a_life_chapter() {
        let suggestions = suggest_chapters(&[("battle", 12), ("siege", 2), ("birth", 1), ("death", 1)]);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].slug, "campaigns");
        assert_eq!(suggestions[0].event_count, 14);
        assert_eq!(suggestions[0].standing, SUGGESTION);
    }

    #[test]
    fn a_philosopher_gets_works_when_the_writings_are_numerous() {
        let suggestions = suggest_chapters(&[("publication", 8), ("birth", 1), ("death", 1)]);
        assert_eq!(suggestions.iter().map(|item| item.slug).collect::<Vec<_>>(), vec!["works"]);
    }

    #[test]
    fn a_sparse_biography_gets_no_chapter() {
        let suggestions = suggest_chapters(&[("birth", 1), ("residence", 1), ("publication", 2)]);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn an_unused_theme_is_not_imposed() {
        let suggestions = suggest_chapters(&[("publication", 6)]);
        assert!(suggestions.iter().all(|item| item.slug != "campaigns"));
        assert!(suggestions.iter().all(|item| item.slug != "exile"));
    }
}
