// crates/talaria-api/src/display_i18n.rs
//! Locale overlays for explorer JSON. Canonical rows stay in source language.

use serde_json::Value;

use crate::llm;

pub fn normalize_ui_lang(raw: Option<&str>) -> Option<&'static str> {
    match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("fr") | Some("fra") | Some("french") => Some("fr"),
        Some("en") | Some("eng") | Some("english") => Some("en"),
        _ => None,
    }
}

pub async fn localize_json_string_fields(values: &mut [Value], lang: &str, keys: &[&str]) {
    let mut originals = Vec::new();
    let mut slots = Vec::new();
    for (vi, value) in values.iter().enumerate() {
        let Some(obj) = value.as_object() else { continue };
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
    tracing::info!(
        lang,
        n = originals.len(),
        "localizing explorer display strings"
    );
    let translated = llm::translate_display_texts(lang, &originals).await;
    for ((vi, key), rendered) in slots.into_iter().zip(translated) {
        if let Some(obj) = values[vi].as_object_mut() {
            obj.insert(key, Value::String(rendered));
        }
    }
}

pub async fn localize_string(lang: &str, text: &str) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }
    llm::translate_display_texts(lang, &[text.to_string()])
        .await
        .into_iter()
        .next()
        .unwrap_or_else(|| text.to_string())
}
