// crates/talaria-sources/src/seeds.rs
//! Generic seed title lists loaded from files — not hardcoded biography rules.

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
}
