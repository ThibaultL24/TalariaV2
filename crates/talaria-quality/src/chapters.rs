// crates/talaria-quality/src/chapters.rs
//! Deterministic chapter suggestions from documented event types.
//! A suggestion is not a draft and not a published chapter.

pub const MIN_CHAPTER_EVENTS: usize = 6;
pub const SUGGESTION: &str = "suggestion";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterSuggestion {
    pub slug: &'static str,
    pub title_fr: &'static str,
    pub title_en: &'static str,
    pub event_count: usize,
    pub standing: &'static str,
    /// The event types that met the threshold. This is the whole justification.
    pub basis_fr: String,
    pub basis_en: String,
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
        let kinds = theme.types.join(", ");
        suggestions.push(ChapterSuggestion {
            slug: theme.slug,
            title_fr: theme.title_fr,
            title_en: theme.title_en,
            event_count,
            standing: SUGGESTION,
            basis_fr: format!("{event_count} faits documentés de type {kinds}"),
            basis_en: format!("{event_count} documented events of type {kinds}"),
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

/// One dated fact used to propose a life period. The place is optional.
#[derive(Debug, Clone)]
pub struct DatedFact {
    pub year: i32,
    pub place: Option<String>,
}

/// A chronological span drawn from the facts themselves. Not a published chapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifePeriod {
    pub start_year: i32,
    pub end_year: i32,
    pub event_count: usize,
    pub title_fr: String,
    pub title_en: String,
    pub basis_fr: String,
    pub basis_en: String,
    pub members: Vec<usize>,
}

const PERIOD_GAP_YEARS: i32 = 8;
/// A longer span stays whole when the facts have no quieter year between them.
const MAX_PERIOD_EVENTS: usize = 36;
const MIN_GAP_INSIDE_A_PERIOD: i32 = 2;
const MIN_PERIOD_EVENTS: usize = 2;
const SPARSE_LIFE: usize = 8;

/// Periods follow time gaps and repeated places. A type bucket stays a category.
pub fn suggest_life_periods(facts: &[DatedFact]) -> Vec<LifePeriod> {
    let mut indexed: Vec<(usize, i32, Option<String>)> = facts
        .iter()
        .enumerate()
        .map(|(index, fact)| {
            let place = fact.place.as_deref().map(str::trim).filter(|place| {
                !place.is_empty()
                    && !crate::is_wikidata_qid(place)
                    && !super::public_admission::is_political_territory(place)
            });
            (index, fact.year, place.map(str::to_string))
        })
        .collect();
    indexed.sort_by_key(|item| item.1);
    if indexed.len() < MIN_PERIOD_EVENTS {
        return Vec::new();
    }
    if indexed.len() < SPARSE_LIFE {
        return vec![period_from(&indexed)];
    }
    let mut groups: Vec<Vec<(usize, i32, Option<String>)>> = Vec::new();
    let mut current = vec![indexed[0].clone()];
    for item in indexed.into_iter().skip(1) {
        let previous_year = current.last().map(|item| item.1).unwrap_or(item.1);
        if item.1 - previous_year >= PERIOD_GAP_YEARS && current.len() >= MIN_PERIOD_EVENTS {
            groups.push(std::mem::take(&mut current));
        }
        current.push(item);
    }
    if current.len() < MIN_PERIOD_EVENTS {
        if let Some(last) = groups.last_mut() {
            last.extend(current);
        } else if current.len() >= MIN_PERIOD_EVENTS {
            groups.push(current);
        }
    } else {
        groups.push(current);
    }
    let mut periods = Vec::new();
    for group in groups {
        split_on_a_quiet_interval(group, &mut periods);
    }
    periods
}

/// Split a crowded span at its widest quiet interval. A continuous year stays one episode.
fn split_on_a_quiet_interval(
    group: Vec<(usize, i32, Option<String>)>,
    periods: &mut Vec<LifePeriod>,
) {
    if group.len() < MIN_PERIOD_EVENTS {
        return;
    }
    if group.len() <= MAX_PERIOD_EVENTS {
        periods.push(period_from(&group));
        return;
    }
    let mut widest: Option<(usize, i32)> = None;
    for index in 1..group.len() {
        let gap = group[index].1 - group[index - 1].1;
        if gap < MIN_GAP_INSIDE_A_PERIOD {
            continue;
        }
        if index < MIN_PERIOD_EVENTS || group.len() - index < MIN_PERIOD_EVENTS {
            continue;
        }
        if widest.is_none_or(|(_, previous)| gap > previous) {
            widest = Some((index, gap));
        }
    }
    let Some((index, _)) = widest else {
        split_busy_years(group, periods);
        return;
    };
    let right = group[index..].to_vec();
    let left = group[..index].to_vec();
    split_on_a_quiet_interval(left, periods);
    split_on_a_quiet_interval(right, periods);
}

const BUSY_YEAR_EVENTS: usize = 8;

/// A year with many facts is its own episode. Quieter years stay together.
fn split_busy_years(
    group: Vec<(usize, i32, Option<String>)>,
    periods: &mut Vec<LifePeriod>,
) {
    let mut quiet = Vec::new();
    let mut index = 0;
    while index < group.len() {
        let year = group[index].1;
        let mut year_end = index;
        while year_end < group.len() && group[year_end].1 == year {
            year_end += 1;
        }
        let bucket = group[index..year_end].to_vec();
        index = year_end;
        if bucket.len() >= BUSY_YEAR_EVENTS {
            if quiet.len() >= MIN_PERIOD_EVENTS {
                periods.push(period_from(&quiet));
            } else if !quiet.is_empty() {
                let mut joined = quiet;
                joined.extend(bucket);
                quiet = Vec::new();
                periods.push(period_from(&joined));
                continue;
            }
            quiet.clear();
            periods.push(period_from(&bucket));
        } else {
            quiet.extend(bucket);
        }
    }
    if quiet.len() >= MIN_PERIOD_EVENTS {
        periods.push(period_from(&quiet));
    } else if !quiet.is_empty() {
        if let Some(last) = periods.last_mut() {
            let mut joined: Vec<(usize, i32, Option<String>)> = group
                .iter()
                .filter(|item| last.members.contains(&item.0))
                .cloned()
                .collect();
            joined.extend(quiet);
            *last = period_from(&joined);
        }
    }
}

fn period_from(group: &[(usize, i32, Option<String>)]) -> LifePeriod {
    let start = group.iter().map(|item| item.1).min().unwrap_or(0);
    let end = group.iter().map(|item| item.1).max().unwrap_or(start);
    let members: Vec<usize> = group.iter().map(|item| item.0).collect();
    let place = dominant_place(group);
    let span_fr = year_span(start, end, true);
    let span_en = year_span(start, end, false);
    LifePeriod {
        start_year: start,
        end_year: end,
        event_count: members.len(),
        title_fr: match &place {
            Some(place) => format!("{span_fr} · {place}"),
            None => span_fr,
        },
        title_en: match &place {
            Some(place) => format!("{span_en} · {place}"),
            None => span_en,
        },
        basis_fr: format!(
            "{} faits entre {} et {}",
            members.len(),
            format_year(start, true),
            format_year(end, true)
        ),
        basis_en: format!(
            "{} facts between {} and {}",
            members.len(),
            format_year(start, false),
            format_year(end, false)
        ),
        members,
    }
}

fn dominant_place(group: &[(usize, i32, Option<String>)]) -> Option<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for (_, _, place) in group {
        let Some(place) = place else { continue };
        if let Some((_, count)) = counts.iter_mut().find(|(name, _)| name == place) {
            *count += 1;
        } else {
            counts.push((place.clone(), 1));
        }
    }
    let placed = counts.iter().map(|(_, count)| *count).sum::<usize>();
    counts.sort_by(|left, right| right.1.cmp(&left.1));
    let (name, count) = counts.first()?;
    (*count >= 2 && *count * 2 >= placed).then(|| name.clone())
}

fn format_year(year: i32, french: bool) -> String {
    if year < 0 {
        if french {
            format!("{} av. n. è.", -year)
        } else {
            format!("{} BCE", -year)
        }
    } else {
        year.to_string()
    }
}

fn year_span(start: i32, end: i32, french: bool) -> String {
    if start == end {
        format_year(start, french)
    } else {
        format!("{}–{}", format_year(start, french), format_year(end, french))
    }
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
    fn a_publication_count_can_suggest_works_without_naming_a_person() {
        let suggestions = suggest_chapters(&[("publication", 8), ("birth", 1), ("death", 1)]);
        assert_eq!(suggestions.iter().map(|item| item.slug).collect::<Vec<_>>(), vec!["works"]);
        assert!(suggestions[0].basis_en.contains("publication"));
        assert!(!suggestions[0].basis_en.contains("battle"));
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

    fn dated(year: i32, place: &str) -> DatedFact {
        DatedFact { year, place: Some(place.to_string()) }
    }

    #[test]
    fn a_long_life_splits_on_time_and_place_without_a_fixed_script() {
        let napoleon = vec![
            dated(1769, "Ajaccio"),
            dated(1779, "Brienne"),
            dated(1784, "Paris"),
            dated(1793, "Toulon"),
            dated(1796, "Lodi"),
            dated(1796, "Mantua"),
            dated(1798, "Alexandria"),
            dated(1799, "Paris"),
            dated(1804, "Paris"),
            dated(1805, "Austerlitz"),
            dated(1812, "Moscow"),
            dated(1814, "Elba"),
            dated(1815, "Waterloo"),
            dated(1815, "Saint Helena"),
            dated(1821, "Saint Helena"),
        ];
        let periods = suggest_life_periods(&napoleon);
        assert!(periods.len() >= 2);
        assert!(periods.iter().any(|period| period.end_year >= 1815));
        assert!(periods.iter().any(|period| period.start_year <= 1769));
        assert!(periods.iter().all(|period| period.event_count <= 12));
        assert!(periods.iter().all(|period| !period.title_en.contains("Campaign of Italy")));
        assert!(periods.iter().all(|period| !period.members.is_empty()));
        let categories = suggest_chapters(&[("battle", 192)]);
        assert_eq!(categories[0].slug, "campaigns");
        assert_ne!(periods[0].title_en, categories[0].title_en);
    }

    #[test]
    fn a_writer_a_scientist_and_an_ancient_life_do_not_share_one_outline() {
        let hugo = vec![
            dated(1822, "Paris"),
            dated(1829, "Paris"),
            dated(1831, "Paris"),
            dated(1841, "Paris"),
            dated(1852, "Guernsey"),
            dated(1856, "Guernsey"),
            dated(1859, "Guernsey"),
            dated(1862, "Guernsey"),
            dated(1870, "Paris"),
            dated(1872, "Paris"),
        ];
        let hugo_periods = suggest_life_periods(&hugo);
        assert!(hugo_periods.iter().any(|period| period.title_en.contains("Guernsey")));
        assert!(hugo_periods.iter().any(|period| period.title_en.contains("Paris")));
        let curie = vec![
            dated(1891, "Paris"),
            dated(1898, "Paris"),
            dated(1903, "Paris"),
            dated(1906, "Paris"),
            dated(1911, "Paris"),
            dated(1914, "Paris"),
        ];
        let curie_periods = suggest_life_periods(&curie);
        assert_eq!(curie_periods.len(), 1);
        assert!(curie_periods[0].title_en.contains("Paris"));
        assert!(!curie_periods[0].title_en.to_lowercase().contains("battle"));
        let aristotle = vec![
            dated(-384, "Stagira"),
            dated(-367, "Athens"),
            dated(-347, "Athens"),
            dated(-335, "Assos"),
            dated(-322, "Chalcis"),
        ];
        let ancient = suggest_life_periods(&aristotle);
        assert_eq!(ancient.len(), 1);
        assert_eq!(ancient[0].start_year, -384);
        assert_eq!(ancient[0].end_year, -322);
        assert!(ancient[0].title_fr.contains("av. n. è."));
        assert!(!ancient[0].title_en.contains("Campaign"));
    }

    #[test]
    fn a_crowd_of_similar_events_is_split_by_coherence() {
        let mut battles: Vec<DatedFact> = (0..8).map(|_| dated(1806, "Jena")).collect();
        battles.extend((0..8).map(|_| dated(1815, "Leipzig")));
        let periods = suggest_life_periods(&battles);
        assert_eq!(periods.len(), 2);
        assert!(periods.iter().all(|period| period.event_count >= 2));
        let continuous: Vec<DatedFact> = (0..20).map(|index| dated(1800 + index / 10, "Leipzig")).collect();
        let one = suggest_life_periods(&continuous);
        assert_eq!(one.len(), 1);
    }
}
