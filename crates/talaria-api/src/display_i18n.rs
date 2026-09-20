// crates/talaria-api/src/display_i18n.rs
//! Locale overlays for explorer JSON. Canonical rows stay in source language.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::Value;

use crate::llm;

pub fn normalize_ui_lang(raw: Option<&str>) -> Option<&'static str> {
    match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("fr") | Some("fra") | Some("french") => Some("fr"),
        Some("en") | Some("eng") | Some("english") => Some("en"),
        _ => None,
    }
}

/// Instant deterministic overlays only (Born/Died/…). Safe for list endpoints.
pub fn localize_json_string_fields_fast(values: &mut [Value], lang: &str, keys: &[&str]) {
    for value in values.iter_mut() {
        let Some(obj) = value.as_object_mut() else {
            continue;
        };
        for key in keys {
            let Some(text) = obj.get(*key).and_then(Value::as_str).map(str::to_string) else {
                continue;
            };
            if text.is_empty() {
                continue;
            }
            if let Some(hit) = deterministic_display_overlay(&text, lang) {
                obj.insert((*key).to_string(), Value::String(hit));
            }
        }
    }
}

/// Localize selected string fields for UI display.
///
/// Strategy (fail-open, never wipe a batch on timeout):
/// 1. Deterministic short templates (Born/Died/office/…) — free, instant
/// 2. Deduplicate remaining strings that need LLM translation
/// 3. Translate unique strings under a wall-clock budget; apply whatever finished
///
/// Prefer [`localize_json_string_fields_fast`] on list endpoints (timeline/geojson/claims) —
/// LLM batch translation can stall the explorer for 10+ seconds.
pub async fn localize_json_string_fields(values: &mut [Value], lang: &str, keys: &[&str]) {
    let mut originals = Vec::new();
    let mut slots = Vec::new();
    for (vi, value) in values.iter().enumerate() {
        let Some(obj) = value.as_object() else {
            continue;
        };
        for key in keys {
            if let Some(text) = obj.get(*key).and_then(Value::as_str) {
                if text.is_empty() {
                    continue;
                }
                slots.push((vi, (*key).to_string()));
                originals.push(text.to_string());
            }
        }
    }
    if originals.is_empty() {
        return;
    }

    // Pass 1: deterministic overlays (and identity for already-target language).
    let mut rendered: Vec<String> = originals
        .iter()
        .map(|text| {
            if let Some(hit) = deterministic_display_overlay(text, lang) {
                hit
            } else {
                text.clone()
            }
        })
        .collect();

    // Pass 2: unique strings still needing LLM.
    let mut unique_order: Vec<String> = Vec::new();
    let mut unique_index: HashMap<String, usize> = HashMap::new();
    for (i, src) in originals.iter().enumerate() {
        if rendered[i] != *src {
            continue; // already deterministically remapped
        }
        if !llm::display_text_needs_translation(src.trim(), lang) {
            continue;
        }
        unique_index.entry(src.clone()).or_insert_with(|| {
            let idx = unique_order.len();
            unique_order.push(src.clone());
            idx
        });
    }

    if unique_order.is_empty() {
        for ((vi, key), text) in slots.into_iter().zip(rendered) {
            if let Some(obj) = values[vi].as_object_mut() {
                obj.insert(key, Value::String(text));
            }
        }
        return;
    }

    tracing::info!(
        lang,
        fields = originals.len(),
        unique = unique_order.len(),
        "localizing explorer display strings"
    );

    // Prefer short titles first so the facts list updates even if summaries lag.
    unique_order.sort_by(|a, b| {
        title_priority(a)
            .cmp(&title_priority(b))
            .then_with(|| a.len().cmp(&b.len()))
    });

    const MAX_UNIQUE: usize = 48;
    if unique_order.len() > MAX_UNIQUE {
        unique_order.truncate(MAX_UNIQUE);
    }

    let translated = llm::translate_display_texts_budgeted(
        lang,
        &unique_order,
        Duration::from_secs(12),
    )
    .await;

    let mut by_src: HashMap<&str, &str> = HashMap::new();
    for (src, dst) in unique_order.iter().zip(translated.iter()) {
        if src != dst {
            by_src.insert(src.as_str(), dst.as_str());
        }
    }

    for (i, src) in originals.iter().enumerate() {
        if let Some(dst) = by_src.get(src.as_str()) {
            rendered[i] = (*dst).to_string();
        }
    }

    for ((vi, key), text) in slots.into_iter().zip(rendered) {
        if let Some(obj) = values[vi].as_object_mut() {
            obj.insert(key, Value::String(text));
        }
    }
}

pub async fn localize_string(lang: &str, text: &str) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }
    if let Some(hit) = deterministic_display_overlay(text, lang) {
        return hit;
    }
    llm::translate_display_texts_budgeted(lang, &[text.to_string()], Duration::from_secs(12))
        .await
        .into_iter()
        .next()
        .unwrap_or_else(|| text.to_string())
}

fn title_priority(text: &str) -> u8 {
    let len = text.chars().count();
    if len <= 80 {
        0
    } else if len <= 180 {
        1
    } else {
        2
    }
}

/// Instant FR↔EN overlays for structured ingest titles (no LLM).
pub fn deterministic_display_overlay(text: &str, target_lang: &str) -> Option<String> {
    let target = if target_lang.starts_with("fr") {
        "fr"
    } else {
        "en"
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(year) = year_after_any_prefix(
        trimmed,
        &["born in ", "né en ", "née en ", "ne en ", "nee en "],
    ) {
        return Some(if target == "fr" {
            format!("Né en {year}")
        } else {
            format!("Born in {year}")
        });
    }
    if let Some(year) = year_after_any_prefix(
        trimmed,
        &[
            "died in ",
            "mort en ",
            "morte en ",
            "décédé en ",
            "décédée en ",
            "decede en ",
            "decedee en ",
        ],
    ) {
        return Some(if target == "fr" {
            format!("Mort en {year}")
        } else {
            format!("Died in {year}")
        });
    }

    // "<kind> · <year>" structured Wikidata titles
    if let Some((kind, year)) = split_dotted_kind_year(trimmed) {
        let label = match (kind.as_str(), target) {
            ("office" | "fonction" | "charge", "fr") => "Fonction",
            ("office" | "fonction" | "charge", _) => "Office",
            ("marriage" | "mariage", "fr") => "Mariage",
            ("marriage" | "mariage", _) => "Marriage",
            ("notable event" | "notable_event" | "événement notable" | "evenement notable", "fr") => {
                "Événement notable"
            }
            ("notable event" | "notable_event" | "événement notable" | "evenement notable", _) => {
                "Notable event"
            }
            ("arrival" | "arrivée" | "arrivee", "fr") => "Arrivée",
            ("arrival" | "arrivée" | "arrivee", _) => "Arrival",
            ("departure" | "départ" | "depart", "fr") => "Départ",
            ("departure" | "départ" | "depart", _) => "Departure",
            ("battle" | "bataille", "fr") => "Bataille",
            ("battle" | "bataille", _) => "Battle",
            ("life_event" | "life event" | "fait de vie", "fr") => "Fait de vie",
            ("life_event" | "life event" | "fait de vie", _) => "Life event",
            _ => return None,
        };
        return Some(format!("{label} · {year}"));
    }

    None
}

fn year_after_any_prefix(text: &str, prefixes: &[&str]) -> Option<String> {
    let lower = text.to_lowercase();
    for prefix in prefixes {
        if let Some(rest) = lower.strip_prefix(prefix) {
            let year = rest.trim();
            if (3..=4).contains(&year.len()) && year.chars().all(|c| c.is_ascii_digit()) {
                return Some(year.to_string());
            }
        }
    }
    None
}

fn split_dotted_kind_year(text: &str) -> Option<(String, String)> {
    let (left, right) = text.split_once('·')?;
    let year = right.trim();
    if !(3..=4).contains(&year.len()) || !year.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let kind = left.trim();
    if kind.is_empty() {
        return None;
    }
    Some((kind.to_lowercase(), year.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn born_died_templates_flip_locale() {
        assert_eq!(
            deterministic_display_overlay("Born in 1890", "fr").as_deref(),
            Some("Né en 1890")
        );
        assert_eq!(
            deterministic_display_overlay("Né en 1890", "en").as_deref(),
            Some("Born in 1890")
        );
        assert_eq!(
            deterministic_display_overlay("Died in 1970", "fr").as_deref(),
            Some("Mort en 1970")
        );
        assert_eq!(
            deterministic_display_overlay("office · 1958", "fr").as_deref(),
            Some("Fonction · 1958")
        );
        assert_eq!(
            deterministic_display_overlay("mariage · 1921", "en").as_deref(),
            Some("Marriage · 1921")
        );
    }

    #[test]
    fn prose_left_to_llm() {
        assert!(deterministic_display_overlay(
            "Charles de Gaulle naît le 22 novembre 1890 à Lille",
            "en"
        )
        .is_none());
    }
}
