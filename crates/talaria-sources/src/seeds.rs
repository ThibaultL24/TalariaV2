// crates/talaria-sources/src/seeds.rs
//! Generic seed title lists loaded from files — not hardcoded biography rules.

use std::collections::HashSet;
use std::path::Path;

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
    match (birth, death) {
        (Some(b), Some(d)) => (b.saturating_sub(2), d.saturating_add(5)),
        (Some(b), None) => (b.saturating_sub(2), b.saturating_add(120)),
        (None, Some(d)) => (d.saturating_sub(120), d.saturating_add(5)),
        (None, None) => (1000, 2100),
    }
}

pub fn first_year_in_window(text: &str, lo: i32, hi: i32) -> Option<String> {
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
    false
}

/// Built-in expansion patterns for linked-page discovery (language-agnostic cues).
pub fn is_high_value_link_title(title: &str) -> bool {
    let lower = title.to_lowercase();
    lower.starts_with("battle of ")
        || lower.starts_with("siege of ")
        || lower.starts_with("treaty of ")
        || lower.starts_with("treaties of ")
        || lower.contains("campaign")
        || lower.contains("war of ")
        || lower.contains("invasion of ")
        || lower.starts_with("list of battles")
        || lower.starts_with("military career")
        || lower.starts_with("early life")
        || lower.starts_with("scientific career")
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
/// Subject page first; surname/high-value links next; noise dropped.
pub fn merge_seed_titles(
    subject: &str,
    existing: impl IntoIterator<Item = String>,
    discovered: impl IntoIterator<Item = String>,
    cap: usize,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifespan_window_is_subject_specific() {
        assert_eq!(lifespan_year_window(Some(1867), Some(1934)), (1865, 1939));
        assert_eq!(lifespan_year_window(Some(1769), Some(1821)), (1767, 1826));
        let (lo, hi) = lifespan_year_window(None, None);
        assert_eq!((lo, hi), (1000, 2100));
        assert!(first_year_in_window("Nobel Prize in 1903 then 1911", 1865, 1939).as_deref() == Some("1903"));
        assert!(first_year_in_window("cited in 2006", 1865, 1939).is_none());
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
}
