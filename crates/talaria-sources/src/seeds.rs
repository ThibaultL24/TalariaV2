// crates/talaria-sources/src/seeds.rs
//! Generic seed title lists loaded from files — not hardcoded biography rules.

use std::collections::HashSet;
use std::path::Path;

use chrono::Datelike;

pub fn load_seed_titles(path: &Path) -> anyhow::Result<Vec<String>> {
    let text = std::fs::read_to_string(path)?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect())
}

/// Year window used by lifespan gates / page-year fallback.
/// Unknown lifespan stays wide (not Napoleon-era 1765–1865).
pub fn lifespan_year_window(birth: Option<i32>, death: Option<i32>) -> (i32, i32) {
    let now = chrono::Utc::now().year();
    match (birth, death) {
        (Some(b), Some(d)) => (b.saturating_sub(2), d.saturating_add(5)),
        (Some(b), None) => (b.saturating_sub(2), now.min(b.saturating_add(120))),
        (None, Some(d)) => (d.saturating_sub(120), d.saturating_add(5)),
        (None, None) => (-4000, now),
    }
}

pub fn first_year_in_window(text: &str, lo: i32, hi: i32) -> Option<String> {
    if let Some(s) = scan_bce_year(text, lo, hi) {
        return Some(s);
    }
    for w in text.split(|c: char| !c.is_ascii_digit()) {
        if w.len() == 4 {
            if let Ok(y) = w.parse::<i32>() {
                if y >= lo && y <= hi {
                    return Some(y.to_string());
                }
            }
        }
    }
    None
}

fn scan_bce_year(text: &str, lo: i32, hi: i32) -> Option<String> {
    let lower = text.to_lowercase();
    let bytes = lower.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let digits = &lower[start..i];
            if digits.len() <= 4 {
                if let Ok(abs) = digits.parse::<i32>() {
                    let rest = lower[i..].trim_start();
                    if rest.starts_with("bc")
                        || rest.starts_with("b.c")
                        || rest.starts_with("bce")
                        || rest.starts_with("av. j")
                        || rest.starts_with("av j")
                    {
                        let y = -abs;
                        if y >= lo && y <= hi {
                            return Some(format!("{abs} BC"));
                        }
                    }
                }
            }
            continue;
        }
        i += 1;
    }
    None
}

pub fn subject_surname(subject: &str) -> Option<String> {
    let last = subject.split_whitespace().last().unwrap_or("");
    let last = last.trim_matches(|c: char| !c.is_alphabetic());
    if last.chars().count() < 3 {
        return None;
    }
    Some(last.to_string())
}

/// Calendar / meta Wikipedia pages that must not enter density exploration.
pub fn is_noise_wiki_title(title: &str) -> bool {
    let t = title.trim();
    if t.len() < 2 || t.len() > 80 {
        return true;
    }
    let lower = t.to_lowercase();
    if lower.starts_with("list of years")
        || lower.starts_with("list of decades")
        || lower.starts_with("list of centuries")
        || lower.starts_with("ad ")
        || lower.starts_with("bc ")
        || lower.ends_with(" in literature")
        || lower.ends_with(" in science")
        || lower.ends_with(" in music")
        || lower.ends_with(" in sports")
        || lower.contains("disambiguation")
        || lower.contains("(identifier)")
        || lower.starts_with("isbn")
        || lower.starts_with("doi")
        || lower.starts_with("category:")
        || lower.starts_with("template:")
        || lower.starts_with("file:")
        || lower.starts_with("wikipedia:")
        || lower.starts_with("help:")
        || lower.starts_with("portal:")
        || lower.starts_with("talk:")
    {
        return true;
    }
    if t.matches(' ').count() > 6 {
        return true;
    }
    if t.contains(':') {
        return true;
    }
    if lower.contains(" the ") && t.matches(' ').count() > 3 {
        return true;
    }
    if t.len() == 4 && t.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    if lower.ends_with(" century") || lower.ends_with(" millennium") {
        return true;
    }
    if (lower.starts_with("list of ") || lower.starts_with("liste des ") || lower.starts_with("liste de "))
        && !is_military_link_title(title)
        && !lower.starts_with("list of awards")
    {
        return true;
    }
    false
}

fn is_military_link_title(title: &str) -> bool {
    let lower = title.to_lowercase();
    lower.starts_with("battle of ")
        || lower.starts_with("bataille de ")
        || lower.starts_with("siege of ")
        || lower.starts_with("siège de ")
        || lower.contains("campaign")
        || lower.contains("war of ")
        || lower.contains("invasion of ")
        || lower.starts_with("list of battles")
        || lower.starts_with("military career")
}

/// Built-in expansion patterns for linked-page discovery (language-agnostic cues).
pub fn is_high_value_link_title(title: &str) -> bool {
    is_military_link_title(title) || is_civil_high_value_link_title(title)
}

fn is_civil_high_value_link_title(title: &str) -> bool {
    let lower = title.to_lowercase();
    lower.starts_with("treaty of ")
        || lower.starts_with("treaties of ")
        || lower.starts_with("early life")
        || lower.starts_with("scientific career")
        || is_life_trace_link_title(title)
        || lower.contains("university")
        || lower.contains("université")
        || lower.contains("institute")
        || lower.contains("institut")
        || lower.contains("laboratory")
        || lower.contains("laboratoire")
        || lower.contains("college")
        || lower.contains("collège")
        || lower.contains("academy")
        || lower.contains("académie")
        || lower.contains("hospital")
        || lower.contains("hôpital")
        || lower.contains("nobel")
        || lower.contains("prize")
        || lower.contains("prix ")
        || lower.contains("radioactiv")
        || lower.starts_with("timeline of ")
        || lower.starts_with("list of awards")
        || lower.contains("conservatoire")
        || lower.contains("polytechnic")
        || lower.contains("polytechnique")
        || lower.contains("sorbonne")
}

/// Linked pages that carry biography geography for any person class.
pub fn is_life_trace_link_title(title: &str) -> bool {
    let lower = title.to_lowercase();
    lower.starts_with("maison de")
        || lower.starts_with("maison de ")
        || lower.starts_with("house of")
        || lower.contains("hiver à")
        || lower.contains("hiver a")
        || lower.contains("winter in")
        || lower.starts_with("voyage")
        || lower.contains("correspondance")
        || lower.contains("correspondence")
        || lower.starts_with("letters of")
        || lower.contains("itinéraire")
        || lower.contains("itinerary")
        || lower.starts_with("early life")
        || lower.starts_with("enfance")
        || lower.contains("childhood of")
        || lower.starts_with("residence of")
        || lower.contains("séjour")
        || lower.contains("sejour")
        || lower.starts_with("château")
        || lower.starts_with("chateau")
        || lower.starts_with("castle of")
}

fn rank_seed_title(subject: &str, title: &str) -> u8 {
    if title.eq_ignore_ascii_case(subject) {
        return 0;
    }
    let lower = title.to_lowercase();
    if let Some(sur) = subject_surname(subject) {
        if lower.contains(&sur.to_lowercase()) {
            return 1;
        }
    }
    if is_high_value_link_title(title) {
        return 2;
    }
    3
}

/// Merge curated seeds with discovered Wikipedia/Wikidata titles.
/// Subject page first; surname and life-trace links (battles, houses, itineraries) next; noise dropped.
/// Person class is not used — the biography page is the map source.
pub fn merge_seed_titles(
    subject: &str,
    existing: impl IntoIterator<Item = String>,
    discovered: impl IntoIterator<Item = String>,
    cap: usize,
) -> Vec<String> {
    merge_seed_titles_for(subject, existing, discovered, cap, true)
}

pub fn merge_seed_titles_for(
    subject: &str,
    existing: impl IntoIterator<Item = String>,
    discovered: impl IntoIterator<Item = String>,
    cap: usize,
    _boost_military_links: bool,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut ranked: Vec<(u8, String)> = Vec::new();
    for title in existing.into_iter().chain(discovered) {
        let title = title.trim().to_string();
        if title.is_empty() || is_noise_wiki_title(&title) {
            continue;
        }
        let key = title.to_lowercase();
        if !seen.insert(key) {
            continue;
        }
        ranked.push((rank_seed_title(subject, &title), title));
    }
    ranked.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    ranked.truncate(cap.max(1));
    ranked.into_iter().map(|(_, t)| t).collect()
}

/// Person bios, museums, and abstract pages — not map/timeline density.
fn looks_like_other_person_or_topic_title(title: &str) -> bool {
    let lower = title.to_lowercase();
    if title.contains('(') && title.chars().any(|c| c.is_ascii_digit()) {
        return true;
    }
    const SKIP: &[&str] = &[
        "musée",
        "musee",
        "museum",
        "peinture",
        "painting",
        "gangrène",
        "gangrene",
        "ministère",
        "ministere",
        "ministres",
    ];
    SKIP.iter().any(|p| lower.contains(p))
}

/// Follow a linked page only when it can add places or dated occurrences.
pub fn is_followable_map_title(title: &str) -> bool {
    if is_noise_wiki_title(title) || looks_like_other_person_or_topic_title(title) {
        return false;
    }
    if is_high_value_link_title(title) {
        return true;
    }
    let lower = title.to_lowercase();
    const GEO: &[&str] = &[
        "bataille",
        "siège",
        "siege",
        "traité",
        "traite",
        "treaty",
        "château",
        "chateau",
        "palais",
        "cathédrale",
        "cathedrale",
        "basilique",
        "hôtel",
        "guerre",
        "paix de",
        "paix des",
        "forteresse",
        "abbaye",
        "citadelle",
        "place de",
    ];
    if GEO.iter().any(|p| lower.contains(p)) {
        return true;
    }
    crate::places::resolve_place_offline(title).is_some()
}

/// Links that sit in the same paragraph as a year — the only related pages worth fetching.
pub fn dated_wikilink_titles(wikitext: &str, lo: i32, hi: i32) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for para in wikitext.split("\n\n") {
        if first_year_in_window(para, lo, hi).is_none() {
            continue;
        }
        for link in talaria_text::extract_wikilinks(para) {
            let title = link.target.trim();
            if title.is_empty() || !is_followable_map_title(title) {
                continue;
            }
            let key = title.to_lowercase();
            if seen.insert(key) {
                out.push(title.to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifespan_window_is_subject_specific() {
        assert_eq!(lifespan_year_window(Some(1867), Some(1934)), (1865, 1939));
        assert_eq!(lifespan_year_window(Some(1769), Some(1821)), (1767, 1826));
        let (lo, hi) = lifespan_year_window(None, None);
        assert_eq!(lo, -4000);
        assert!(hi >= 2026);
        let (lo2, hi2) = lifespan_year_window(Some(1977), None);
        assert_eq!(lo2, 1975);
        assert!(hi2 >= 2026 && hi2 <= 1977 + 120);
        assert!(first_year_in_window("Nobel Prize in 1903 then 1911", 1865, 1939).as_deref() == Some("1903"));
        assert!(first_year_in_window("cited in 2006", 1865, 1939).is_none());
        assert_eq!(
            first_year_in_window("died in 30 BC in Alexandria", -4000, 2100).as_deref(),
            Some("30 BC")
        );
    }

    #[test]
    fn generic_seeds_prefer_subject_and_drop_noise() {
        let merged = merge_seed_titles(
            "Marie Curie",
            ["Marie Curie".into()],
            [
                "1903".into(),
                "20th century".into(),
                "University of Paris".into(),
                "Pierre Curie".into(),
                "Nobel Prize in Physics".into(),
                "Category:French physicists".into(),
            ],
            10,
        );
        assert_eq!(merged[0], "Marie Curie");
        assert!(merged.iter().any(|t| t == "Pierre Curie"));
        assert!(merged.iter().any(|t| t.contains("Nobel")));
        assert!(merged.iter().any(|t| t.contains("University")));
        assert!(!merged.iter().any(|t| t == "1903" || t.contains("century") || t.starts_with("Category:")));
    }

    #[test]
    fn life_trace_merge_keeps_battles_and_palaces_without_a_person_class() {
        let merged = merge_seed_titles(
            "Louis XIV",
            ["Louis XIV".into()],
            [
                "Bataille de Rocroi".into(),
                "Château de Versailles".into(),
                "List of Belgian football clubs".into(),
                "20th century".into(),
            ],
            8,
        );
        assert_eq!(merged[0], "Louis XIV");
        assert!(merged.iter().any(|t| t.contains("Rocroi")), "{merged:?}");
        assert!(merged.iter().any(|t| t.contains("Versailles")), "{merged:?}");
        assert!(!merged.iter().any(|t| t.contains("football") || t.contains("century")));
    }

    #[test]
    fn dated_links_come_from_the_paragraph_not_a_demo_list() {
        let wt = "En 1654 Louis XIV est sacré à [[Cathédrale Notre-Dame de Reims]] après la [[Bataille de Rethel]].\n\nVoir aussi [[Football]].\n\n[[Bataille de Rocroi]] sans date ici.";
        let titles = dated_wikilink_titles(wt, 1638, 1715);
        assert!(titles.iter().any(|t| t.contains("Reims")), "{titles:?}");
        assert!(titles.iter().any(|t| t.contains("Rethel")), "{titles:?}");
        assert!(!titles.iter().any(|t| t.contains("Football") || t.contains("Rocroi")));
    }

    #[test]
    fn followable_map_titles_skip_other_people_and_museums() {
        assert!(is_followable_map_title("Château de Versailles"));
        assert!(is_followable_map_title("Bataille de Fleurus"));
        assert!(is_followable_map_title("Traité des Pyrénées"));
        assert!(!is_followable_map_title("Anne d'Autriche (1601-1666)"));
        assert!(!is_followable_map_title("Musée du Louvre"));
        assert!(!is_followable_map_title("Jules Mazarin"));
        assert!(!is_followable_map_title("Gangrène"));
    }

    #[test]
    fn life_trace_titles_are_high_value_for_any_person() {
        assert!(is_life_trace_link_title("Maison de George Sand"));
        assert!(is_life_trace_link_title("Un hiver à Majorque"));
        assert!(is_life_trace_link_title("Letters of Ada Lovelace"));
        assert!(is_high_value_link_title("Un hiver à Majorque"));
        assert!(!is_life_trace_link_title("List of Belgian football clubs"));
    }
}
