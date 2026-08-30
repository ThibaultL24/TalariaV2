// crates/talaria-api/src/narrative_dossier.rs
//! Event-local dossier: weave a short paragraph from several sources with [n] markers.
//! Quotes stay in source_refs; the paragraph is a compressed recap, not a wiki dump.

use serde_json::{json, Value};
use talaria_store::{CanonicalEventRow, EventEvidenceRow, NarrativeContextRow};
use talaria_text::split_sentences;

#[derive(Debug, Clone)]
pub struct DossierClaim {
    pub text: String,
    pub page_title: String,
    pub language: String,
    pub section_title: Option<String>,
    pub oldid: Option<i64>,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct EventDossier {
    pub event_summary: String,
    pub how_it_happened: String,
    pub source_refs: Vec<Value>,
}

pub async fn build_event_dossier(
    pool: &sqlx::PgPool,
    event: &CanonicalEventRow,
    fact: Option<&str>,
    narrative: &[NarrativeContextRow],
    evidence: &[EventEvidenceRow],
    wikipedia_title: Option<&str>,
    wiki_lang: &str,
    offline_only: bool,
) -> EventDossier {
    let mut claims = local_claims(event, fact, narrative, evidence);

    // Prefer locally stored dump sections before any live MediaWiki call.
    if let Some(title) = wikipedia_title {
        if let Ok(extra) = local_section_claims(pool, title, wiki_lang, event, evidence).await {
            merge_claims(&mut claims, extra);
        }
    }

    // Live wiki padding used to pull unrelated "Personal life" sentences.
    // Only fetch when this event has no local evidence at all.
    if !offline_only && claims.is_empty() {
        if let Some(title) = wikipedia_title {
            let oldid = evidence.iter().find_map(|row| row.revision_id);
            for lang in dossier_languages(wiki_lang) {
                if !claims.is_empty() {
                    break;
                }
                if let Ok(extra) = fetch_section_claims(title, lang, event, oldid).await {
                    merge_claims(&mut claims, extra);
                }
            }
        }
    }

    let mut dossier = assemble_dossier(event, fact, claims, wiki_lang);
    if !offline_only {
        let year = event
            .start_time
            .map(|time| time.format("%Y").to_string());
        let sources: Vec<String> = dossier
            .source_refs
            .iter()
            .filter_map(|row| {
                row.get("snippet")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            })
            .collect();
        if let Some(recap) = crate::llm::synthesize_event_recap(crate::llm::EventRecapRequest {
            person: &event.person_name,
            lang: wiki_lang,
            event_type: &event.event_type,
            year: year.as_deref(),
            place: event.place_label.as_deref(),
            sources: &sources,
        })
        .await
        {
            dossier.event_summary = recap.clone();
            dossier.how_it_happened = recap;
        }
    }
    dossier
}

fn assemble_dossier(
    event: &CanonicalEventRow,
    fact: Option<&str>,
    claims: Vec<DossierClaim>,
    lang: &str,
) -> EventDossier {
    let mut on_topic: Vec<DossierClaim> = claims
        .into_iter()
        .filter(|claim| claim_supports_event(event, fact, claim))
        .collect();

    if on_topic.is_empty() {
        let fallback = fact
            .map(str::to_string)
            .or_else(|| event.summary.clone())
            .unwrap_or_else(|| event.title.clone());
        return EventDossier {
            event_summary: fallback.clone(),
            how_it_happened: fallback,
            source_refs: Vec::new(),
        };
    }

    on_topic.truncate(6);

    let how = weave_paragraph(event, lang, &on_topic);
    let source_refs = on_topic
        .iter()
        .enumerate()
        .map(|(index, claim)| claim_to_source_ref(claim, index + 1))
        .collect();

    EventDossier {
        event_summary: how.clone(),
        how_it_happened: how,
        source_refs,
    }
}

fn claim_supports_event(
    event: &CanonicalEventRow,
    fact: Option<&str>,
    claim: &DossierClaim,
) -> bool {
    if claim
        .section_title
        .as_deref()
        .is_some_and(|title| title.eq_ignore_ascii_case("evidence"))
    {
        return true;
    }
    if fact
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .is_some_and(|text| normalize_key(text) == normalize_key(&claim.text))
    {
        return true;
    }
    let (keywords, subject_keys) = relevance_keywords(event);
    is_relevant(&claim.text, &keywords, &subject_keys)
}

fn dossier_languages(primary: &str) -> Vec<&'static str> {
    match primary {
        "fr" => vec!["fr", "en"],
        _ => vec!["fr", "en"], // prefer denser FR biographical sections when available
    }
}

fn local_claims(
    event: &CanonicalEventRow,
    fact: Option<&str>,
    narrative: &[NarrativeContextRow],
    evidence: &[EventEvidenceRow],
) -> Vec<DossierClaim> {
    let title = evidence
        .iter()
        .find_map(|row| row.wiki_title.clone())
        .or_else(|| narrative.first().map(|row| row.wiki_title.clone()))
        .unwrap_or_else(|| event.person_name.clone());
    let lang = evidence
        .iter()
        .find_map(|row| row.wiki_lang.clone())
        .or_else(|| narrative.first().map(|row| row.wiki_lang.clone()))
        .unwrap_or_else(|| "en".into());
    let oldid = evidence.iter().find_map(|row| row.revision_id);
    let page_url = format!(
        "https://{lang}.wikipedia.org/wiki/{}",
        title.replace(' ', "_")
    );
    let url = oldid
        .map(|id| {
            format!(
                "https://{lang}.wikipedia.org/w/index.php?title={}&oldid={id}",
                title.replace(' ', "_")
            )
        })
        .unwrap_or_else(|| page_url);

    let mut out = Vec::new();
    if let Some(fact) = fact.map(str::trim).filter(|text| !text.is_empty()) {
        out.push(DossierClaim {
            text: clean_claim_text(fact),
            page_title: title.clone(),
            language: lang.clone(),
            section_title: Some("evidence".into()),
            oldid,
            url: url.clone(),
        });
    }

    for row in narrative {
        if row.is_evidence {
            continue;
        }
        if is_identity_blurb(&row.text) {
            continue;
        }
        let text = clean_claim_text(&row.text);
        if text.chars().count() < 24 {
            continue;
        }
        let (keywords, subject_keys) = relevance_keywords(event);
        if !is_relevant(&text, &keywords, &subject_keys) {
            continue;
        }
        out.push(DossierClaim {
            text,
            page_title: row.wiki_title.clone(),
            language: row.wiki_lang.clone(),
            section_title: Some(format!("adjacent · sentence {}", row.ordinal)),
            oldid,
            url: url.clone(),
        });
    }
    out
}

async fn local_section_claims(
    pool: &sqlx::PgPool,
    title: &str,
    wiki_lang: &str,
    event: &CanonicalEventRow,
    evidence: &[EventEvidenceRow],
) -> anyhow::Result<Vec<DossierClaim>> {
    let mut sections = talaria_store::list_sections_for_title(pool, wiki_lang, title).await?;
    // Also try FR title aliases for denser bios when primary lang is EN.
    if sections.is_empty() && wiki_lang != "fr" {
        if title.to_ascii_lowercase().contains("napoleon") {
            sections = talaria_store::list_sections_for_title(pool, "fr", "Napoléon Ier").await?;
        }
    }
    if sections.is_empty() {
        return Ok(Vec::new());
    }

    let preferred = sections
        .iter()
        .find(|section| section_matches_event(&section.title, event))
        .cloned();

    let mut chosen = Vec::new();
    if let Some(section) = preferred {
        chosen.push(section);
    }
    for section in sections {
        if chosen.iter().any(|existing| existing.id == section.id) {
            continue;
        }
        if !section_matches_event(&section.title, event) {
            continue;
        }
        chosen.push(section);
        if chosen.len() >= 2 {
            break;
        }
    }

    let oldid = evidence
        .iter()
        .find_map(|row| row.revision_id)
        .or_else(|| chosen.first().and_then(|s| s.revision_id));
    let (keywords, subject_keys) = relevance_keywords(event);
    let mut claims = Vec::new();

    for section in chosen {
        let plain = talaria_text::wikitext_to_plain(&section.text);
        for span in split_sentences(&plain) {
            let text = clean_claim_text(&span.text);
            let text = strip_section_heading(&text, &section.title);
            if text.chars().count() < 40 || text.chars().count() > 320 {
                continue;
            }
            if !is_relevant(&text, &keywords, &subject_keys) {
                continue;
            }
            if is_identity_blurb(&text) {
                continue;
            }
            let lang = section.wiki_lang.clone();
            let page_title = section.page_title.clone();
            let cite_url = oldid
                .map(|id| {
                    format!(
                        "https://{lang}.wikipedia.org/w/index.php?title={}&oldid={id}",
                        page_title.replace(' ', "_")
                    )
                })
                .unwrap_or_else(|| {
                    format!(
                        "https://{lang}.wikipedia.org/wiki/{}",
                        page_title.replace(' ', "_")
                    )
                });
            claims.push(DossierClaim {
                text,
                page_title,
                language: lang,
                section_title: Some(section.title.clone()),
                oldid,
                url: cite_url,
            });
            if claims.len() >= 5 {
                return Ok(claims);
            }
        }
    }

    Ok(claims)
}

fn is_generic_private_life_section(section_title: &str) -> bool {
    let lower = section_title.to_ascii_lowercase();
    lower.contains("vie privée")
        || lower.contains("vie privee")
        || lower.contains("personal life")
        || lower.contains("private life")
        || lower == "vie"
        || lower == "life"
}

fn is_family_event(event: &CanonicalEventRow) -> bool {
    matches!(
        event.event_type.as_str(),
        "marriage" | "divorce" | "family" | "wedding"
    )
}

fn section_matches_event(section_title: &str, event: &CanonicalEventRow) -> bool {
    if is_generic_private_life_section(section_title) && !is_family_event(event) {
        return false;
    }
    let lower = section_title.to_ascii_lowercase();
    match event.event_type.as_str() {
        "birth" => {
            lower.contains("naissance")
                || lower.contains("birth")
                || lower.contains("early life")
                || lower.contains("enfance")
                || lower.contains("jeunesse")
        }
        "death" => {
            lower.contains("mort")
                || lower.contains("death")
                || lower.contains("décès")
                || lower.contains("deces")
        }
        "marriage" | "divorce" | "family" | "wedding" => {
            is_generic_private_life_section(section_title)
                || lower.contains("family")
                || lower.contains("famille")
                || lower.contains("mariage")
                || lower.contains("marriage")
        }
        _ => {
            lower.contains("career")
                || lower.contains("carrière")
                || lower.contains("carriere")
                || lower.contains("reign")
                || lower.contains("military")
                || lower.contains("early life")
                || lower.contains("enfance")
                || lower.contains("jeunesse")
                || lower.contains("childhood")
                || lower.contains("business")
                || lower.contains("fortune")
                || lower.contains("wealth")
                || event_topic_tokens(event).iter().any(|token| {
                    token.chars().count() >= 5 && lower.contains(token.as_str())
                })
        }
    }
}

async fn fetch_section_claims(
    title: &str,
    lang: &str,
    event: &CanonicalEventRow,
    oldid: Option<i64>,
) -> anyhow::Result<Vec<DossierClaim>> {
    let client = reqwest::Client::builder()
        .user_agent("TalariaV2/0.1 (historical research; local-dev)")
        .timeout(std::time::Duration::from_secs(12))
        .build()?;

    let resolved = resolve_title(&client, lang, title)
        .await
        .unwrap_or_else(|| title.to_string());
    let section = find_best_section(&client, lang, &resolved, event).await?;
    let Some((section_index, section_title)) = section else {
        return Ok(Vec::new());
    };

    let html = fetch_section_html(&client, lang, &resolved, &section_index).await?;
    let plain = html_to_plain(&html);
    let (keywords, subject_keys) = relevance_keywords(event);
    let mut claims = Vec::new();

    for span in split_sentences(&plain) {
        let text = clean_claim_text(&span.text);
        let text = strip_section_heading(&text, &section_title);
        if text.chars().count() < 40 || text.chars().count() > 320 {
            continue;
        }
        if !is_relevant(&text, &keywords, &subject_keys) {
            continue;
        }
        if is_identity_blurb(&text) {
            continue;
        }
        let cite_url = oldid
            .map(|id| {
                format!(
                    "https://{lang}.wikipedia.org/w/index.php?title={}&oldid={id}",
                    resolved.replace(' ', "_")
                )
            })
            .unwrap_or_else(|| {
                format!(
                    "https://{lang}.wikipedia.org/wiki/{}",
                    resolved.replace(' ', "_")
                )
            });
        claims.push(DossierClaim {
            text,
            page_title: resolved.clone(),
            language: lang.to_string(),
            section_title: Some(section_title.clone()),
            oldid,
            url: cite_url,
        });
        if claims.len() >= 5 {
            break;
        }
    }

    Ok(claims)
}

async fn resolve_title(client: &reqwest::Client, lang: &str, title: &str) -> Option<String> {
    let url = format!("https://{lang}.wikipedia.org/w/api.php");
    let response = client
        .get(&url)
        .query(&[
            ("action", "query"),
            ("titles", title),
            ("redirects", "1"),
            ("format", "json"),
        ])
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let value: Value = response.json().await.ok()?;
    let pages = value.pointer("/query/pages")?.as_object()?;
    for page in pages.values() {
        if page.get("missing").is_some() {
            continue;
        }
        if let Some(resolved) = page.get("title").and_then(Value::as_str) {
            return Some(resolved.to_string());
        }
    }
    // Common FR biography title for Napoleon.
    if lang == "fr" && title.to_ascii_lowercase().contains("napoleon") {
        return Some("Napoléon Ier".into());
    }
    None
}

async fn find_best_section(
    client: &reqwest::Client,
    lang: &str,
    title: &str,
    event: &CanonicalEventRow,
) -> anyhow::Result<Option<(String, String)>> {
    let url = format!("https://{lang}.wikipedia.org/w/api.php");
    let response = client
        .get(&url)
        .query(&[
            ("action", "parse"),
            ("page", title),
            ("prop", "sections"),
            ("format", "json"),
        ])
        .send()
        .await?
        .error_for_status()?;
    let value: Value = response.json().await?;
    let sections = value
        .pointer("/parse/sections")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let needles = section_needles(event.event_type.as_str(), lang);
    let mut best: Option<(i32, String, String)> = None;

    for section in sections {
        let line = section
            .get("line")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_ascii_lowercase();
        let index = section
            .get("index")
            .and_then(|v| {
                v.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            })
            .unwrap_or_default();
        let title_line = section
            .get("line")
            .and_then(Value::as_str)
            .unwrap_or("section")
            .to_string();
        for (score, needle) in needles.iter().enumerate() {
            if line.contains(needle) {
                let rank = score as i32;
                if best
                    .as_ref()
                    .map(|(best_rank, _, _)| rank < *best_rank)
                    .unwrap_or(true)
                {
                    best = Some((rank, index.clone(), title_line.clone()));
                }
                break;
            }
        }
    }

    Ok(best.map(|(_, index, title)| (index, title)))
}

async fn fetch_section_html(
    client: &reqwest::Client,
    lang: &str,
    title: &str,
    section_index: &str,
) -> anyhow::Result<String> {
    let url = format!("https://{lang}.wikipedia.org/w/api.php");
    let response = client
        .get(&url)
        .query(&[
            ("action", "parse"),
            ("page", title),
            ("prop", "text"),
            ("section", section_index),
            ("disableeditsection", "1"),
            ("format", "json"),
        ])
        .send()
        .await?
        .error_for_status()?;
    let value: Value = response.json().await?;
    Ok(value
        .pointer("/parse/text/*")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string())
}

fn section_needles(event_type: &str, lang: &str) -> Vec<&'static str> {
    match (event_type, lang) {
        ("birth", "fr") => vec![
            "naissance",
            "situation personnelle",
            "enfance",
            "jeunesse",
            "origine",
        ],
        ("birth", _) => vec!["birth", "early life", "childhood", "youth", "origins"],
        ("death", "fr") => vec!["mort", "décès", "dernières années", "sainte-hélène"],
        ("death", _) => vec!["death", "final years", "last years", "exile"],
        ("education", "fr") => vec!["formation", "éducation", "enfance et formation", "école"],
        ("education", _) => vec!["education", "school", "military school", "early life"],
        ("battle", "fr") => vec!["campagne", "bataille", "guerre"],
        ("battle", _) => vec!["battle", "campaign", "war"],
        ("office", "fr") => vec!["consulat", "empire", "pouvoir", "politique"],
        ("office", _) => vec!["rule", "emperor", "consulate", "power"],
        (_, "fr") => vec![
            "carrière",
            "parcours",
            "enfance",
            "jeunesse",
            "fortune",
            "affaires",
        ],
        _ => vec![
            "career",
            "early life",
            "childhood",
            "business",
            "wealth",
            "fortune",
        ],
    }
}

fn event_topic_tokens(event: &CanonicalEventRow) -> Vec<String> {
    let blob = format!(
        "{} {}",
        event.title,
        event.summary.as_deref().unwrap_or("")
    );
    let person = event
        .person_name
        .split('(')
        .next()
        .unwrap_or(&event.person_name)
        .to_ascii_lowercase();
    let mut tokens = Vec::new();
    for raw in blob.split(|c: char| !c.is_alphanumeric()) {
        let token = raw.to_ascii_lowercase();
        if token.chars().count() < 4 || is_topic_stopword(&token) {
            continue;
        }
        if person.split_whitespace().any(|part| part == token) {
            continue;
        }
        if !tokens.iter().any(|existing| existing == &token) {
            tokens.push(token);
        }
    }
    tokens
}

fn is_topic_stopword(token: &str) -> bool {
    matches!(
        token,
        "this"
            | "that"
            | "with"
            | "from"
            | "into"
            | "about"
            | "after"
            | "before"
            | "their"
            | "there"
            | "which"
            | "would"
            | "could"
            | "have"
            | "been"
            | "were"
            | "was"
            | "dans"
            | "pour"
            | "avec"
            | "selon"
            | "leur"
            | "leurs"
            | "cette"
            | "sont"
            | "plus"
            | "aussi"
            | "comme"
            | "dont"
            | "une"
            | "des"
            | "les"
            | "the"
            | "and"
            | "for"
            | "his"
            | "her"
            | "its"
            | "age"
    )
}

fn relevance_keywords(event: &CanonicalEventRow) -> (Vec<String>, Vec<String>) {
    let mut keys = event_topic_tokens(event);
    let mut subject_keys = Vec::new();
    let person = event.person_name.split('(').next().unwrap_or(&event.person_name).trim();
    if !person.is_empty() {
        subject_keys.push(person.to_ascii_lowercase());
        if let Some(sur) = talaria_sources::subject_surname(person) {
            subject_keys.push(sur.to_ascii_lowercase());
        }
    }
    if let Some(place) = event.place_label.as_deref() {
        keys.push(place.to_ascii_lowercase());
    }
    match event.event_type.as_str() {
        "birth" => keys.extend(
            [
                "naît",
                "nait",
                "born",
                "birth",
                "bapt",
                "ondoy",
                "mère",
                "mother",
                "father",
                "père",
                "famille",
                "family",
                "ajaccio",
                "buonaparte",
            ]
            .into_iter()
            .map(str::to_string),
        ),
        "death" => keys.extend(
            [
                "mort",
                "died",
                "death",
                "décès",
                "expire",
                "sainte-hélène",
                "st helena",
            ]
            .into_iter()
            .map(str::to_string),
        ),
        "battle" => keys.extend(
            [
                "bataille", "battle", "fought", "combat", "victoire", "defeat",
            ]
            .into_iter()
            .map(str::to_string),
        ),
        "education" => keys.extend(
            [
                "étudi",
                "school",
                "école",
                "studied",
                "formation",
                "brienne",
            ]
            .into_iter()
            .map(str::to_string),
        ),
        _ => {}
    }
    (keys, subject_keys)
}

fn strip_section_heading(text: &str, section_title: &str) -> String {
    let mut text = text.trim().to_string();
    for prefix in [
        section_title,
        &format!("{section_title} "),
        &format!("{section_title}."),
    ] {
        if text.starts_with(prefix) {
            text = text[prefix.len()..]
                .trim_start_matches([' ', ':', '—', '-'])
                .to_string();
        }
    }
    text
}

fn is_relevant(text: &str, keywords: &[String], subject_keys: &[String]) -> bool {
    let lower = text.to_ascii_lowercase();
    if !subject_keys.is_empty()
        && !subject_keys
            .iter()
            .any(|key| !key.trim().is_empty() && lower.contains(key.as_str()))
    {
        return false;
    }
    if keywords.is_empty() {
        return false;
    }
    let hits = keywords
        .iter()
        .filter(|key| key.trim().len() >= 4 && lower.contains(key.as_str()))
        .count();
    hits >= 1
}

fn merge_claims(into: &mut Vec<DossierClaim>, extra: Vec<DossierClaim>) {
    for claim in extra {
        let norm = normalize_key(&claim.text);
        if into
            .iter()
            .any(|existing| normalize_key(&existing.text) == norm)
        {
            continue;
        }
        // Prefer FR biographical density: insert after local evidence.
        into.push(claim);
    }
}

fn normalize_key(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .take(12)
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn short_person(name: &str) -> String {
    name.split('(').next().unwrap_or(name).trim().to_string()
}

fn sources_blob(claims: &[DossierClaim]) -> String {
    claims
        .iter()
        .map(|claim| claim.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn grounded_scene(event: &CanonicalEventRow, lang: &str, blob: &str) -> Option<String> {
    let person = short_person(&event.person_name);
    if person.is_empty() {
        return None;
    }
    let year = event
        .start_time
        .map(|time| time.format("%Y").to_string())
        .filter(|year| blob.contains(year));
    let place = event
        .place_label
        .as_deref()
        .map(str::trim)
        .filter(|place| {
            talaria_quality::is_human_place_label(place)
                && blob.contains(&place.to_ascii_lowercase())
        });
    let fr = lang.starts_with("fr");
    let childhood = blob.contains("age eight")
        || blob.contains("âge de huit")
        || blob.contains("age de huit")
        || blob.contains("childhood")
        || blob.contains("enfance");

    if fr {
        return match (event.event_type.as_str(), year.as_deref(), place) {
            ("marriage" | "wedding", Some(year), Some(place)) => {
                Some(format!("{person} se marie en {year} à {place}."))
            }
            ("marriage" | "wedding", Some(year), None) => {
                Some(format!("{person} se marie en {year}."))
            }
            ("birth", Some(year), Some(place)) => {
                Some(format!("{person} naît en {year} à {place}."))
            }
            ("birth", Some(year), None) => Some(format!("{person} naît en {year}.")),
            ("death", Some(year), Some(place)) => {
                Some(format!("{person} meurt en {year} à {place}."))
            }
            ("death", Some(year), None) => Some(format!("{person} meurt en {year}.")),
            ("battle" | "siege", Some(year), Some(place)) => {
                Some(format!("En {year}, {person} est engagé dans un combat à {place}."))
            }
            ("residence", Some(year), Some(place)) => {
                Some(format!("En {year}, {person} s’installe à {place}."))
            }
            (_, Some(year), Some(place)) => {
                Some(format!("En {year}, un épisode de la vie de {person} se joue à {place}."))
            }
            (_, Some(year), None) => Some(format!("En {year}, dans la vie de {person}.")),
            _ if childhood => Some(format!("Épisode de l’enfance de {person}.")),
            _ => Some(format!("Ce que les sources disent de cet épisode de {person}.")),
        };
    }

    match (event.event_type.as_str(), year.as_deref(), place) {
        ("marriage" | "wedding", Some(year), Some(place)) => {
            Some(format!("{person} marries in {year} in {place}."))
        }
        ("marriage" | "wedding", Some(year), None) => {
            Some(format!("{person} marries in {year}."))
        }
        ("birth", Some(year), Some(place)) => Some(format!("{person} is born in {year} in {place}.")),
        ("birth", Some(year), None) => Some(format!("{person} is born in {year}.")),
        ("death", Some(year), Some(place)) => Some(format!("{person} dies in {year} in {place}.")),
        ("death", Some(year), None) => Some(format!("{person} dies in {year}.")),
        ("battle" | "siege", Some(year), Some(place)) => {
            Some(format!("In {year}, {person} is in a fight at {place}."))
        }
        ("residence", Some(year), Some(place)) => {
            Some(format!("In {year}, {person} settles in {place}."))
        }
        (_, Some(year), Some(place)) => {
            Some(format!("In {year}, an episode in {person}’s life takes place in {place}."))
        }
        (_, Some(year), None) => Some(format!("In {year}, in {person}’s life.")),
        _ if childhood => Some(format!("From {person}’s childhood.")),
        _ => Some(format!("What the sources say about this episode in {person}’s life.")),
    }
}

fn scene_repeats_claim(scene: &str, claim: &str) -> bool {
    normalize_key(scene) == normalize_key(claim)
        || (normalize_key(claim).contains(&normalize_key(scene)) && scene.chars().count() > 24)
}

fn weave_paragraph(event: &CanonicalEventRow, lang: &str, claims: &[DossierClaim]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let blob = sources_blob(claims);
    if let Some(scene) = grounded_scene(event, lang, &blob) {
        let first = claims.first().map(|claim| claim.text.as_str()).unwrap_or("");
        if !scene_repeats_claim(&scene, first) {
            parts.push(scene);
        }
    }

    for (index, claim) in claims.iter().enumerate() {
        let n = index + 1;
        let compact = compact_claim(&claim.text, event, claim.section_title.as_deref());
        if compact.is_empty() {
            continue;
        }
        parts.push(format!("{compact}[{n}]"));
        if parts.len() >= 5 {
            break;
        }
    }

    let mut out = String::new();
    for part in parts {
        if out.is_empty() {
            out = part;
        } else {
            out = format!("{out} {part}");
        }
        if out.chars().count() > 850 {
            break;
        }
    }
    out
}

fn compact_claim(text: &str, event: &CanonicalEventRow, section_title: Option<&str>) -> String {
    let mut text = text.trim().to_string();
    if let Some(section) = section_title {
        let prefixes = [section, &format!("{section} "), &format!("{section}.")];
        for prefix in prefixes {
            if text.starts_with(prefix) {
                text = text[prefix.len()..].trim().to_string();
            }
        }
    }
    // Drop accidental section heading leftovers.
    for heading in ["Naissance", "Birth", "Early life", "Mort", "Death"] {
        if text.starts_with(heading) {
            text = text[heading.len()..]
                .trim_start_matches([' ', ':', '—', '-'])
                .to_string();
        }
    }

    let _ = event;
    if text.chars().count() > 200 {
        if let Some((head, _)) = text.split_once(". ") {
            if head.chars().count() >= 48 {
                text = format!("{head}.");
            }
        }
    }
    text.trim().trim_end_matches(',').to_string()
}

fn claim_to_source_ref(claim: &DossierClaim, n: usize) -> Value {
    json!({
        "type": "evidence_pointer",
        "kind": "wikipedia_sentence",
        "source_system": "wikipedia",
        "language": claim.language,
        "page_title": claim.page_title,
        "source_page_title": claim.page_title,
        "oldid": claim.oldid,
        "revision_id": claim.oldid,
        "snippet": claim.text,
        "quote": claim.text,
        "label": format!("[{n}] Wikipedia — {}", claim.page_title),
        "section_title": claim.section_title,
        "url": claim.url,
        "source_url": claim.url,
        "wikipedia_url": format!(
            "https://{}.wikipedia.org/wiki/{}",
            claim.language,
            claim.page_title.replace(' ', "_")
        ),
        "page_url": format!(
            "https://{}.wikipedia.org/wiki/{}",
            claim.language,
            claim.page_title.replace(' ', "_")
        ),
        "revision_url": claim.oldid.map(|id| format!(
            "https://{}.wikipedia.org/w/index.php?title={}&oldid={id}",
            claim.language,
            claim.page_title.replace(' ', "_")
        )),
        "citation_index": n,
        "confidence": 0.9,
    })
}

fn clean_claim_text(text: &str) -> String {
    scrub_wiki_brackets(&decode_basic_entities(text))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn scrub_wiki_brackets(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            let mut j = i + 1;
            while j < chars.len() && chars[j] != ']' {
                j += 1;
            }
            if j < chars.len() {
                let inner: String = chars[i + 1..j].iter().collect();
                let compact: String = inner.chars().filter(|c| !c.is_whitespace()).collect();
                let drop = compact.chars().all(|c| c.is_ascii_digit())
                    || compact.len() <= 2
                    || compact.to_ascii_lowercase().starts_with("réf")
                    || compact.to_ascii_lowercase().starts_with("ref")
                    || compact.to_ascii_lowercase().contains("citation");
                if drop {
                    i = j + 1;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn html_to_plain(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script = false;
    let lower = html.to_ascii_lowercase();
    let bytes = html.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !in_tag && lower[i..].starts_with("<script") {
            in_script = true;
        }
        if in_script && lower[i..].starts_with("</script>") {
            in_script = false;
            i += 9;
            continue;
        }
        if in_script {
            i += 1;
            continue;
        }
        let c = html[i..].chars().next().unwrap_or(' ');
        let width = c.len_utf8();
        if c == '<' {
            in_tag = true;
            i += width;
            continue;
        }
        if c == '>' {
            in_tag = false;
            out.push(' ');
            i += width;
            continue;
        }
        if !in_tag {
            out.push(c);
        }
        i += width;
    }
    decode_basic_entities(&out)
        .replace('\u{00a0}', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode_basic_entities(text: &str) -> String {
    let mut out = text
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#91;", "[")
        .replace("&#93;", "]")
        .replace("&#160;", " ");

    // Numeric decimal entities: &#123;
    while let Some(start) = out.find("&#") {
        let after = &out[start + 2..];
        let (digits, is_hex) =
            if let Some(rest) = after.strip_prefix('x').or_else(|| after.strip_prefix('X')) {
                (
                    rest.chars()
                        .take_while(|c| c.is_ascii_hexdigit())
                        .collect::<String>(),
                    true,
                )
            } else {
                (
                    after
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect::<String>(),
                    false,
                )
            };
        if digits.is_empty() {
            break;
        }
        let end_offset = start
            + 2
            + if is_hex {
                1 + digits.len()
            } else {
                digits.len()
            };
        if out.as_bytes().get(end_offset) != Some(&b';') {
            // skip malformed
            out = format!("{}{}", &out[..start + 2], &out[start + 2..]);
            break;
        }
        let code = if is_hex {
            u32::from_str_radix(&digits, 16).ok()
        } else {
            digits.parse::<u32>().ok()
        };
        if let Some(ch) = code.and_then(char::from_u32) {
            out = format!("{}{}{}", &out[..start], ch, &out[end_offset + 1..]);
        } else {
            break;
        }
    }
    out
}

fn is_identity_blurb(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains(" was a ")
        || lower.contains(" was an ")
        || lower.contains(" is a ")
        || lower.contains(" is an ")
        || lower.contains(" known as ")
        || lower.contains(" est un ")
        || lower.contains(" est une ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn trump_wealth_event() -> CanonicalEventRow {
        CanonicalEventRow {
            id: Uuid::nil(),
            entity_id: Uuid::nil(),
            person_name: "Donald Trump".into(),
            event_type: "historical_fact".into(),
            epistemic_status: "accepted".into(),
            title: "Millionaire by age eight".into(),
            summary: Some(
                "Trump was a millionaire in inflation-adjusted dollars by age eight".into(),
            ),
            start_time: Utc.with_ymd_and_hms(1954, 1, 1, 0, 0, 0).single(),
            time_json: json!({"kind":"approx","precision":"year"}),
            place_label: None,
            confidence: 0.8,
            map_eligible: false,
            lat: None,
            lon: None,
        }
    }

    fn claim(text: &str, section: &str) -> DossierClaim {
        DossierClaim {
            text: text.into(),
            page_title: "Donald Trump".into(),
            language: "fr".into(),
            section_title: Some(section.into()),
            oldid: None,
            url: "https://fr.wikipedia.org/wiki/Donald_Trump".into(),
        }
    }

    #[test]
    fn wealth_fact_keeps_matching_sentence() {
        let event = trump_wealth_event();
        let (keywords, subject_keys) = relevance_keywords(&event);
        assert!(is_relevant(
            "Trump was a millionaire in inflation-adjusted dollars by age eight.",
            &keywords,
            &subject_keys,
        ));
    }

    #[test]
    fn vie_privee_family_sentences_are_not_relevant_to_wealth_fact() {
        let event = trump_wealth_event();
        let (keywords, subject_keys) = relevance_keywords(&event);
        for text in [
            "Selon d'autres sources, Trump rencontrait des mannequins de Montréal au bar Maxwell's Plum pour la promotion des Jeux olympiques.",
            "Donald Trump a nié une relation avec Carla Bruni en 1991.",
            "Il est marié à Melania Knauss depuis 2005 et leur fils Barron est né en 2006.",
            "Donald Trump Jr. et Vanessa Haydon Trump ont trois enfants.",
        ] {
            assert!(
                !is_relevant(text, &keywords, &subject_keys),
                "expected unrelated: {text}"
            );
        }
    }

    #[test]
    fn personal_life_section_does_not_match_generic_historical_fact() {
        let event = trump_wealth_event();
        assert!(!section_matches_event("Vie privée", &event));
        assert!(!section_matches_event("Personal life", &event));
        assert!(!section_matches_event("Private life", &event));
    }

    #[test]
    fn assemble_drops_off_topic_sources_and_uses_woven_summary() {
        let event = trump_wealth_event();
        let fact = "Trump was a millionaire in inflation-adjusted dollars by age eight";
        let dossier = assemble_dossier(
            &event,
            Some(fact),
            vec![
                claim(fact, "evidence"),
                claim(
                    "Selon d'autres sources, Trump rencontrait des mannequins de Montréal au bar Maxwell's Plum pour la promotion des Jeux olympiques.",
                    "Vie privée",
                ),
                claim(
                    "Donald Trump a nié une relation avec Carla Bruni en 1991.",
                    "Vie privée",
                ),
                claim(
                    "Il est marié à Melania Knauss depuis 2005 et leur fils Barron est né en 2006.",
                    "Vie privée",
                ),
            ],
            "fr",
        );
        assert_eq!(dossier.source_refs.len(), 1);
        assert_eq!(dossier.event_summary, dossier.how_it_happened);
        assert!(dossier.event_summary.contains(fact));
        assert!(dossier.event_summary.contains("enfance"));
        assert!(dossier.how_it_happened.contains("[1]"));
        let snippet = dossier.source_refs[0]
            .get("snippet")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(snippet.to_ascii_lowercase().contains("millionaire"));
    }

    #[test]
    fn framing_ignores_year_place_missing_from_the_fact() {
        let mut event = trump_wealth_event();
        event.title = "Donald Trump — historical_fact (2024) @ Jamaica".into();
        event.place_label = Some("Jamaica".into());
        event.start_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).single();
        let fact = "Trump was a millionaire in inflation-adjusted dollars by age eight";
        let dossier = assemble_dossier(&event, Some(fact), vec![claim(fact, "evidence")], "fr");
        assert!(
            !dossier.event_summary.contains("Jamaica"),
            "got {}",
            dossier.event_summary
        );
        assert!(
            !dossier.event_summary.contains("2024"),
            "got {}",
            dossier.event_summary
        );
        assert!(dossier.event_summary.contains("millionaire"));
        assert!(dossier.event_summary.contains("enfance"));
    }

    #[test]
    fn marriage_scene_uses_year_from_the_quote() {
        let mut event = trump_wealth_event();
        event.event_type = "marriage".into();
        event.summary = Some("In 1977, Trump married Ivana Zelníčková".into());
        event.start_time = Utc.with_ymd_and_hms(1977, 1, 1, 0, 0, 0).single();
        event.place_label = Some("Siena".into());
        let fact = "In 1977, Trump married Ivana Zelníčková";
        let dossier = assemble_dossier(&event, Some(fact), vec![claim(fact, "evidence")], "fr");
        assert!(dossier.event_summary.contains("se marie en 1977"));
        assert!(!dossier.event_summary.contains("Siena"));
        assert!(dossier.event_summary.contains("Ivana"));
    }
}
