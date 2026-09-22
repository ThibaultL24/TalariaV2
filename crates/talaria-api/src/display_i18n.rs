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

fn target_lang(raw: &str) -> &'static str {
    if raw.starts_with("fr") {
        "fr"
    } else {
        "en"
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

/// Timeline / GeoJSON list items: always emit titles and places in `lang`.
///
/// Uses event_type + year + place when the stored Wikipedia sentence is in the
/// other language — no LLM (list endpoints must stay instant).
pub fn localize_event_list_items(values: &mut [Value], lang: &str) {
    let lang = target_lang(lang);
    for value in values.iter_mut() {
        let Some(obj) = value.as_object_mut() else {
            continue;
        };
        let event_type = obj
            .get("event_type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let title = obj
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let place = obj
            .get("place_label")
            .and_then(Value::as_str)
            .map(str::to_string);
        let year = year_from_event_obj(obj);

        if let Some(place) = place.as_deref() {
            let localized = localize_place_label(place, lang);
            if localized != place {
                obj.insert("place_label".into(), Value::String(localized));
            }
        }
        let place_now = obj
            .get("place_label")
            .and_then(Value::as_str)
            .map(str::to_string);

        let new_title = localize_list_title(
            lang,
            &event_type,
            year,
            place_now.as_deref(),
            &title,
        );
        if !new_title.is_empty() && new_title != title {
            obj.insert("title".into(), Value::String(new_title));
        }
    }
}

fn localize_list_title(
    lang: &str,
    event_type: &str,
    year: Option<i32>,
    place: Option<&str>,
    stored: &str,
) -> String {
    let trimmed = stored.trim();
    if keep_stored_prose(trimmed, lang) {
        return trimmed.to_string();
    }
    if !event_type.is_empty() {
        return structured_headline(lang, event_type, year, place);
    }
    deterministic_display_overlay(trimmed, lang).unwrap_or_else(|| trimmed.to_string())
}

fn keep_stored_prose(stored: &str, lang: &str) -> bool {
    if stored.is_empty() || stored.contains('·') {
        return false;
    }
    if born_died_overlay(stored, lang).is_some() {
        return false;
    }
    !llm::display_text_needs_translation(stored, lang)
}

fn structured_headline(
    lang: &str,
    event_type: &str,
    year: Option<i32>,
    place: Option<&str>,
) -> String {
    let key = canonical_event_type(event_type).unwrap_or(event_type);
    let place = place
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(|text| localize_place_label(text, lang));

    if key == "birth" {
        return born_died_headline(lang, true, year, place.as_deref());
    }
    if key == "death" {
        return born_died_headline(lang, false, year, place.as_deref());
    }

    let label = event_type_label(key, lang)
        .unwrap_or_else(|| humanize_type(event_type, lang));
    join_headline(&label, year, place.as_deref())
}

fn born_died_headline(lang: &str, born: bool, year: Option<i32>, place: Option<&str>) -> String {
    let fr = lang == "fr";
    let place = place.map(|p| localize_place_label(p, lang));
    let place = place.as_deref();
    match (born, year, place) {
        (true, Some(year), Some(place)) => {
            if fr {
                format!("Né à {place} en {year}")
            } else {
                format!("Born in {place} in {year}")
            }
        }
        (false, Some(year), Some(place)) => {
            if fr {
                format!("Mort à {place} en {year}")
            } else {
                format!("Died in {place} in {year}")
            }
        }
        (true, Some(year), None) => {
            if fr {
                format!("Né en {year}")
            } else {
                format!("Born in {year}")
            }
        }
        (false, Some(year), None) => {
            if fr {
                format!("Mort en {year}")
            } else {
                format!("Died in {year}")
            }
        }
        (true, None, Some(place)) => {
            if fr {
                format!("Né à {place}")
            } else {
                format!("Born in {place}")
            }
        }
        (false, None, Some(place)) => {
            if fr {
                format!("Mort à {place}")
            } else {
                format!("Died in {place}")
            }
        }
        (true, None, None) => {
            if fr {
                "Naissance".into()
            } else {
                "Birth".into()
            }
        }
        (false, None, None) => {
            if fr {
                "Mort".into()
            } else {
                "Death".into()
            }
        }
    }
}

fn join_headline(label: &str, year: Option<i32>, place: Option<&str>) -> String {
    let mut parts = vec![label.to_string()];
    if let Some(year) = year {
        parts.push(year.to_string());
    }
    if let Some(place) = place.filter(|text| !text.is_empty()) {
        parts.push(place.to_string());
    }
    parts.join(" · ")
}

fn year_from_event_obj(obj: &serde_json::Map<String, Value>) -> Option<i32> {
    if let Some(start) = obj
        .get("time")
        .and_then(|time| time.get("start"))
        .and_then(Value::as_str)
    {
        if let Some(year) = parse_year_prefix(start) {
            return Some(year);
        }
    }
    obj.get("start_time")
        .and_then(Value::as_str)
        .and_then(parse_year_prefix)
}

fn parse_year_prefix(text: &str) -> Option<i32> {
    let digits: String = text
        .chars()
        .skip_while(|c| *c == '-')
        .take(4)
        .filter(|c| c.is_ascii_digit())
        .collect();
    if digits.len() == 4 {
        digits.parse().ok()
    } else {
        None
    }
}

/// Localize selected string fields for UI display.
///
/// Strategy (fail-open, never wipe a batch on timeout):
/// 1. Deterministic short templates (Born/Died/office/…) — free, instant
/// 2. Deduplicate remaining strings that need LLM translation
/// 3. Translate unique strings under a wall-clock budget; apply whatever finished
///
/// Prefer [`localize_event_list_items`] on list endpoints (timeline/geojson) —
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
pub fn deterministic_display_overlay(text: &str, lang: &str) -> Option<String> {
    let target = target_lang(lang);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let as_place = localize_place_label(trimmed, target);
    if as_place != trimmed {
        return Some(as_place);
    }

    if let Some(hit) = born_died_overlay(trimmed, target) {
        return Some(hit);
    }

    dotted_kind_overlay(trimmed, target)
}

fn born_died_overlay(text: &str, target: &str) -> Option<String> {
    let lower = text.to_lowercase();

    if let Some((place, year)) = parse_born_died(
        &lower,
        &[
            "born in ",
            "né en ",
            "née en ",
            "ne en ",
            "nee en ",
            "né à ",
            "née à ",
            "ne a ",
            "nee a ",
        ],
    ) {
        return Some(born_died_headline(target, true, year, place.as_deref()));
    }
    if let Some((place, year)) = parse_born_died(
        &lower,
        &[
            "died in ",
            "mort en ",
            "morte en ",
            "mort à ",
            "morte à ",
            "décédé en ",
            "décédée en ",
            "decede en ",
            "decedee en ",
            "décédé à ",
            "décédée à ",
        ],
    ) {
        return Some(born_died_headline(target, false, year, place.as_deref()));
    }
    None
}

/// `(optional place, optional year)` after a born/died prefix.
fn parse_born_died(lower: &str, prefixes: &[&str]) -> Option<(Option<String>, Option<i32>)> {
    for prefix in prefixes {
        let Some(rest) = lower.strip_prefix(prefix) else {
            continue;
        };
        let rest = rest.trim();
        if rest.is_empty() {
            continue;
        }
        if let Some(year) = parse_plain_year(rest) {
            return Some((None, Some(year)));
        }
        // "Ajaccio in 1769" / "Ajaccio en 1769" / "Ajaccio, 1769"
        if let Some((place, year)) = split_place_year(rest) {
            return Some((Some(place), Some(year)));
        }
        if rest.chars().count() <= 48 {
            return Some((Some(restore_place_case(rest)), None));
        }
    }
    None
}

fn parse_plain_year(text: &str) -> Option<i32> {
    let year = text.trim();
    if (3..=4).contains(&year.len()) && year.chars().all(|c| c.is_ascii_digit()) {
        year.parse().ok()
    } else {
        None
    }
}

fn split_place_year(rest: &str) -> Option<(String, i32)> {
    for sep in [" in ", " en ", ", "] {
        if let Some((place, year_raw)) = rest.rsplit_once(sep) {
            let place = place.trim();
            if place.is_empty() {
                continue;
            }
            if let Some(year) = parse_plain_year(year_raw) {
                return Some((restore_place_case(place), year));
            }
        }
    }
    None
}

fn restore_place_case(place: &str) -> String {
    // Input was lowercased; re-title-case simple toponyms.
    place
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn dotted_kind_overlay(text: &str, target: &str) -> Option<String> {
    if !text.contains('·') {
        return None;
    }
    let parts: Vec<&str> = text
        .split('·')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() < 2 {
        return None;
    }

    let mut year: Option<i32> = None;
    let mut kind: Option<&str> = None;
    let mut place: Option<&str> = None;
    for part in &parts {
        if let Some(parsed) = parse_plain_year(part) {
            year = Some(parsed);
            continue;
        }
        if kind.is_none() && canonical_event_type(part).is_some() {
            kind = Some(*part);
            continue;
        }
        if place.is_none() {
            place = Some(*part);
        }
    }
    let kind = kind?;
    let key = canonical_event_type(kind)?;
    let place = place.map(|p| localize_place_label(p, target));
    if key == "birth" || key == "death" {
        return Some(born_died_headline(
            target,
            key == "birth",
            year,
            place.as_deref(),
        ));
    }
    let label = event_type_label(key, target)?;
    Some(join_headline(&label, year, place.as_deref()))
}

fn humanize_type(event_type: &str, lang: &str) -> String {
    if let Some(label) = event_type_label(event_type, lang) {
        return label;
    }
    event_type.replace('_', " ")
}

fn canonical_event_type(surface: &str) -> Option<&'static str> {
    let key = surface
        .trim()
        .to_lowercase()
        .replace('_', " ")
        .replace('-', " ");
    let key = key.replace("é", "e").replace("è", "e").replace("ê", "e");
    match key.as_str() {
        "birth" | "naissance" => Some("birth"),
        "death" | "mort" | "deces" => Some("death"),
        "marriage" | "mariage" | "wedding" => Some("marriage"),
        "divorce" => Some("divorce"),
        "education" | "etudes" | "study" => Some("education"),
        "employment" | "emploi" => Some("employment"),
        "relocation" | "demenagement" => Some("relocation"),
        "travel" | "voyage" => Some("travel"),
        "residence" => Some("residence"),
        "exile" | "exil" => Some("exile"),
        "imprisonment" | "emprisonnement" => Some("imprisonment"),
        "battle" | "bataille" => Some("battle"),
        "siege" => Some("siege"),
        "diplomatic" | "diplomatie" => Some("diplomatic"),
        "meeting" | "rencontre" => Some("meeting"),
        "office" | "fonction" | "charge" => Some("office"),
        "speech" | "discours" => Some("speech"),
        "award" | "distinction" => Some("award"),
        "publication" => Some("publication"),
        "creation" => Some("creation"),
        "discovery" | "decouverte" => Some("discovery"),
        "anecdote" => Some("anecdote"),
        "statue" => Some("statue"),
        "museum" | "musee" => Some("museum"),
        "street naming" | "odonymie" => Some("street_naming"),
        "memorial" => Some("memorial"),
        "life event" | "fait de vie" => Some("life_event"),
        "historical fact" | "fait historique" => Some("historical_fact"),
        "trial" | "proces" => Some("trial"),
        "legal event" | "fait juridique" => Some("legal_event"),
        "political event" | "fait politique" => Some("political_event"),
        "health event" | "sante" => Some("health_event"),
        "financial event" | "argent" => Some("financial_event"),
        "censorship" | "censure" => Some("censorship"),
        "notable event" | "evenement notable" => Some("notable_event"),
        "arrival" | "arrivee" => Some("arrival"),
        "departure" | "depart" => Some("departure"),
        _ => None,
    }
}

fn event_type_label(key: &str, lang: &str) -> Option<String> {
    let key = canonical_event_type(key).unwrap_or(key);
    let fr = lang == "fr";
    let label = match key {
        "birth" => {
            if fr {
                "Naissance"
            } else {
                "Birth"
            }
        }
        "death" => {
            if fr {
                "Mort"
            } else {
                "Death"
            }
        }
        "marriage" => {
            if fr {
                "Mariage"
            } else {
                "Marriage"
            }
        }
        "divorce" => "Divorce",
        "education" => {
            if fr {
                "Études"
            } else {
                "Education"
            }
        }
        "employment" => {
            if fr {
                "Emploi"
            } else {
                "Employment"
            }
        }
        "relocation" => {
            if fr {
                "Déménagement"
            } else {
                "Relocation"
            }
        }
        "travel" => {
            if fr {
                "Voyage"
            } else {
                "Travel"
            }
        }
        "residence" => {
            if fr {
                "Résidence"
            } else {
                "Residence"
            }
        }
        "exile" => {
            if fr {
                "Exil"
            } else {
                "Exile"
            }
        }
        "imprisonment" => {
            if fr {
                "Emprisonnement"
            } else {
                "Imprisonment"
            }
        }
        "battle" => {
            if fr {
                "Bataille"
            } else {
                "Battle"
            }
        }
        "siege" => {
            if fr {
                "Siège"
            } else {
                "Siege"
            }
        }
        "diplomatic" => {
            if fr {
                "Diplomatie"
            } else {
                "Diplomatic"
            }
        }
        "meeting" => {
            if fr {
                "Rencontre"
            } else {
                "Meeting"
            }
        }
        "office" => {
            if fr {
                "Charge"
            } else {
                "Office"
            }
        }
        "speech" => {
            if fr {
                "Discours"
            } else {
                "Speech"
            }
        }
        "award" => {
            if fr {
                "Distinction"
            } else {
                "Award"
            }
        }
        "publication" => "Publication",
        "creation" => {
            if fr {
                "Création"
            } else {
                "Creation"
            }
        }
        "discovery" => {
            if fr {
                "Découverte"
            } else {
                "Discovery"
            }
        }
        "anecdote" => "Anecdote",
        "statue" => "Statue",
        "museum" => {
            if fr {
                "Musée"
            } else {
                "Museum"
            }
        }
        "street_naming" => {
            if fr {
                "Odonymie"
            } else {
                "Street naming"
            }
        }
        "memorial" => {
            if fr {
                "Mémorial"
            } else {
                "Memorial"
            }
        }
        "life_event" => {
            if fr {
                "Fait de vie"
            } else {
                "Life event"
            }
        }
        "historical_fact" => {
            if fr {
                "Fait historique"
            } else {
                "Historical fact"
            }
        }
        "trial" => {
            if fr {
                "Procès"
            } else {
                "Trial"
            }
        }
        "legal_event" => {
            if fr {
                "Fait juridique"
            } else {
                "Legal event"
            }
        }
        "political_event" => {
            if fr {
                "Fait politique"
            } else {
                "Political event"
            }
        }
        "health_event" => {
            if fr {
                "Santé"
            } else {
                "Health"
            }
        }
        "financial_event" => {
            if fr {
                "Argent"
            } else {
                "Money"
            }
        }
        "censorship" => {
            if fr {
                "Censure"
            } else {
                "Censorship"
            }
        }
        "notable_event" => {
            if fr {
                "Événement notable"
            } else {
                "Notable event"
            }
        }
        "arrival" => {
            if fr {
                "Arrivée"
            } else {
                "Arrival"
            }
        }
        "departure" => {
            if fr {
                "Départ"
            } else {
                "Departure"
            }
        }
        _ => return None,
    };
    Some(label.to_string())
}

/// Gazetteer FR↔EN for common historical places. Unknown labels pass through.
pub fn localize_place_label(place: &str, lang: &str) -> String {
    let target = target_lang(lang);
    let trimmed = place.trim();
    if trimmed.is_empty() {
        return trimmed.to_string();
    }
    let needle = trimmed.to_lowercase();
    for (en, fr) in PLACE_PAIRS {
        if needle == en.to_lowercase() || needle == fr.to_lowercase() {
            return if target == "fr" {
                (*fr).to_string()
            } else {
                (*en).to_string()
            };
        }
    }
    trimmed.to_string()
}

/// Well-known person labels that differ between EN and FR (search + roster).
pub fn localize_person_label(label: &str, lang: &str) -> String {
    let target = target_lang(lang);
    let needle = label.trim().to_lowercase();
    if needle.is_empty() {
        return label.to_string();
    }
    for (en, fr) in PERSON_PAIRS {
        if needle == en.to_lowercase() || needle == fr.to_lowercase() {
            return if target == "fr" {
                (*fr).to_string()
            } else {
                (*en).to_string()
            };
        }
    }
    label.trim().to_string()
}

const PERSON_PAIRS: &[(&str, &str)] = &[
    ("Napoleon", "Napoléon"),
    ("Napoleon Bonaparte", "Napoléon Bonaparte"),
    ("Joan of Arc", "Jeanne d'Arc"),
    ("Moliere", "Molière"),
];

const PLACE_PAIRS: &[(&str, &str)] = &[
    ("Moscow", "Moscou"),
    ("Vienna", "Vienne"),
    ("Saint Helena", "Sainte-Hélène"),
    ("St Helena", "Sainte-Hélène"),
    ("Corsica", "Corse"),
    ("Egypt", "Égypte"),
    ("London", "Londres"),
    ("Venice", "Venise"),
    ("Genoa", "Gênes"),
    ("Naples", "Naples"),
    ("Mantua", "Mantoue"),
    ("Florence", "Florence"),
    ("Rome", "Rome"),
    ("Milan", "Milan"),
    ("Turin", "Turin"),
    ("Lisbon", "Lisbonne"),
    ("The Hague", "La Haye"),
    ("Brussels", "Bruxelles"),
    ("Antwerp", "Anvers"),
    ("Cologne", "Cologne"),
    ("Munich", "Munich"),
    ("Warsaw", "Varsovie"),
    ("Prague", "Prague"),
    ("Cairo", "Le Caire"),
    ("Alexandria", "Alexandrie"),
    ("Jerusalem", "Jérusalem"),
    ("Athens", "Athènes"),
    ("Constantinople", "Constantinople"),
    ("Elba", "Île d'Elbe"),
    ("Jena", "Iéna"),
    ("Dresden", "Dresde"),
    ("Leipzig", "Leipzig"),
    ("Spain", "Espagne"),
    ("Italy", "Italie"),
    ("Germany", "Allemagne"),
    ("England", "Angleterre"),
    ("France", "France"),
    ("Austria", "Autriche"),
    ("Poland", "Pologne"),
    ("Russia", "Russie"),
    ("Prussia", "Prusse"),
    ("Sicily", "Sicile"),
    ("Tuscany", "Toscane"),
    ("Piedmont", "Piémont"),
    ("Savoy", "Savoie"),
    ("Brittany", "Bretagne"),
    ("Normandy", "Normandie"),
    ("Netherlands", "Pays-Bas"),
    ("Belgium", "Belgique"),
    ("Switzerland", "Suisse"),
    ("United States", "États-Unis"),
    ("New York", "New York"),
    ("Washington", "Washington"),
    ("Ajaccio", "Ajaccio"),
    ("Waterloo", "Waterloo"),
    ("Austerlitz", "Austerlitz"),
    ("Paris", "Paris"),
    ("Brienne-le-Château", "Brienne-le-Château"),
    ("Saint Helena Island", "Île de Sainte-Hélène"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
            Some("Charge · 1958")
        );
        assert_eq!(
            deterministic_display_overlay("mariage · 1921", "en").as_deref(),
            Some("Marriage · 1921")
        );
        assert_eq!(
            deterministic_display_overlay("battle · 1815 · Waterloo", "fr").as_deref(),
            Some("Bataille · 1815 · Waterloo")
        );
        assert_eq!(
            deterministic_display_overlay("Moscow", "fr").as_deref(),
            Some("Moscou")
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

    #[test]
    fn list_items_flip_foreign_prose_to_structured_headline() {
        let mut values = vec![json!({
            "event_type": "battle",
            "title": "Napoleon fought and was defeated at Waterloo in 1815.",
            "place_label": "Waterloo",
            "start_time": "1815-06-18T00:00:00Z",
        })];
        localize_event_list_items(&mut values, "fr");
        assert_eq!(
            values[0].get("title").and_then(Value::as_str),
            Some("Bataille · 1815 · Waterloo")
        );
    }

    #[test]
    fn list_items_keep_same_language_prose() {
        let mut values = vec![json!({
            "event_type": "battle",
            "title": "Napoléon est vaincu à Waterloo en 1815.",
            "place_label": "Waterloo",
            "start_time": "1815-06-18T00:00:00Z",
        })];
        localize_event_list_items(&mut values, "fr");
        assert_eq!(
            values[0].get("title").and_then(Value::as_str),
            Some("Napoléon est vaincu à Waterloo en 1815.")
        );
    }

    #[test]
    fn list_items_localize_birth_and_place() {
        let mut values = vec![json!({
            "event_type": "birth",
            "title": "Born in 1769",
            "place_label": "Ajaccio",
            "start_time": "1769-08-15T00:00:00Z",
        })];
        localize_event_list_items(&mut values, "fr");
        assert_eq!(
            values[0].get("title").and_then(Value::as_str),
            Some("Né à Ajaccio en 1769")
        );
    }
}
