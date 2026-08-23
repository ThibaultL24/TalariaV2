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

    if !offline_only && claims.len() < 4 {
        if let Some(title) = wikipedia_title {
            let oldid = evidence.iter().find_map(|row| row.revision_id);
            for lang in dossier_languages(wiki_lang) {
                if claims.len() >= 6 {
                    break;
                }
                if let Ok(extra) = fetch_section_claims(title, lang, event, oldid).await {
                    merge_claims(&mut claims, extra);
                }
            }
        }
    }

    if claims.is_empty() {
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

    claims.truncate(6);

    // Prefer a dense FR biographical section when we have enough claims from it.
    let fr_section: Vec<DossierClaim> = claims
        .iter()
        .filter(|claim| {
            claim.language == "fr"
                && claim
                    .section_title
                    .as_deref()
                    .map(|title| {
                        let lower = title.to_ascii_lowercase();
                        lower.contains("naissance")
                            || lower.contains("mort")
                            || lower.contains("enfance")
                            || lower.contains("jeunesse")
                            || lower.contains("early")
                            || lower.contains("birth")
                            || lower.contains("death")
                    })
                    .unwrap_or(false)
        })
        .cloned()
        .collect();

    let weave_claims = if fr_section.len() >= 3 {
        fr_section
    } else {
        claims.clone()
    };

    let how = weave_paragraph(event, &weave_claims);
    let summary = fact
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            weave_claims
                .first()
                .map(|c| c.text.clone())
                .unwrap_or_default()
        });

    // Keep the full claim pool in source_refs (local + section), numbered as woven first.
    let mut ordered = weave_claims.clone();
    for claim in claims {
        let norm = normalize_key(&claim.text);
        if ordered
            .iter()
            .any(|existing| normalize_key(&existing.text) == norm)
        {
            continue;
        }
        ordered.push(claim);
    }
    ordered.truncate(8);

    let source_refs = ordered
        .iter()
        .enumerate()
        .map(|(index, claim)| claim_to_source_ref(claim, index + 1))
        .collect();

    EventDossier {
        event_summary: summary,
        how_it_happened: how,
        source_refs,
    }
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
        if section_matches_event(&section.title, event) || chosen.len() < 2 {
            chosen.push(section);
        }
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

fn section_matches_event(section_title: &str, event: &CanonicalEventRow) -> bool {
    let lower = section_title.to_ascii_lowercase();
    match event.event_type.as_str() {
        "birth" => {
            lower.contains("naissance")
                || lower.contains("birth")
                || lower.contains("early")
                || lower.contains("enfance")
                || lower.contains("jeunesse")
        }
        "death" => {
            lower.contains("mort")
                || lower.contains("death")
                || lower.contains("décès")
                || lower.contains("deces")
        }
        _ => {
            lower.contains("career")
                || lower.contains("carrière")
                || lower.contains("carriere")
                || lower.contains("reign")
                || lower.contains("military")
                || lower.contains("life")
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
        (_, "fr") => vec!["biographie", "vie", "parcours"],
        _ => vec!["life", "biography", "career"],
    }
}

fn relevance_keywords(event: &CanonicalEventRow) -> (Vec<String>, Vec<String>) {
    let mut keys = Vec::new();
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
    if let Some(time) = event.start_time {
        keys.push(time.format("%Y").to_string());
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
        _ => keys.extend(["en ", "à ", "in ", "at "].into_iter().map(str::to_string)),
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
    let hits = keywords
        .iter()
        .filter(|key| !key.trim().is_empty() && lower.contains(key.as_str()))
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

fn weave_paragraph(event: &CanonicalEventRow, claims: &[DossierClaim]) -> String {
    let lang = claims
        .first()
        .map(|claim| claim.language.as_str())
        .unwrap_or("en");
    let mut parts: Vec<String> = Vec::new();

    // Editorial lead — not a biography dump.
    if let Some(lead) = framing_sentence(event, lang) {
        parts.push(lead);
    }

    for (index, claim) in claims.iter().enumerate() {
        let n = index + 1;
        let compact = compact_claim(&claim.text, event, claim.section_title.as_deref());
        if compact.is_empty() {
            continue;
        }
        if n == 1 && parts.len() == 1 && claim_covers_framing(parts[0].as_str(), &compact) {
            parts.clear();
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

fn framing_sentence(event: &CanonicalEventRow, lang: &str) -> Option<String> {
    let year = event.start_time.map(|t| t.format("%Y").to_string());
    let place = event
        .place_label
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let person = event.person_name.as_str();

    if lang == "fr" {
        return match (event.event_type.as_str(), year.as_deref(), place) {
            ("birth", Some(year), Some(place)) => Some(format!(
                "{person} — naissance : les sources situent l’événement à {place} en {year}."
            )),
            ("death", Some(year), Some(place)) => Some(format!(
                "{person} — mort : les sources situent l’événement à {place} en {year}."
            )),
            ("battle", Some(year), Some(place)) => Some(format!(
                "{person} — bataille : les sources décrivent l’affrontement à {place} en {year}."
            )),
            (_, Some(year), Some(place)) => Some(format!(
                "{person} — les sources situent cet épisode à {place} en {year}."
            )),
            _ => None,
        };
    }

    match (event.event_type.as_str(), year.as_deref(), place) {
        ("birth", Some(year), Some(place)) => Some(format!(
            "{person} — birth: sources place the event in {place} in {year}."
        )),
        ("death", Some(year), Some(place)) => Some(format!(
            "{person} — death: sources place the event in {place} in {year}."
        )),
        (_, Some(year), Some(place)) => Some(format!(
            "{person} — sources situate this episode in {place} in {year}."
        )),
        _ => None,
    }
}

fn claim_covers_framing(lead: &str, claim: &str) -> bool {
    let lead_l = lead.to_ascii_lowercase();
    let claim_l = claim.to_ascii_lowercase();
    let year = lead_l.split_whitespace().find_map(|word| {
        let trimmed: String = word.chars().filter(|c| c.is_ascii_digit()).collect();
        (trimmed.len() == 4).then_some(trimmed)
    });
    let has_year = year.map(|y| claim_l.contains(&y)).unwrap_or(false);
    has_year
        && (claim_l.contains("naît")
            || claim_l.contains("nait")
            || claim_l.contains("born")
            || claim_l.contains("mort")
            || claim_l.contains("died"))
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
