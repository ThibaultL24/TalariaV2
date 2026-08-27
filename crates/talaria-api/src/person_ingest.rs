// crates/talaria-api/src/person_ingest.rs
//! One ingest: dump Wikipedia → grounded LLM → pipeline='person' facts + Agora debates.

use std::collections::HashSet;
use std::path::Path;

use chrono::{TimeZone, Utc};
use serde_json::{json, Value};
use talaria_core::AppConfig;
use talaria_quality::{
    accept_items, occurrence_key_for_event, GroundedItem, Lane, RawExtractItem, TypedTime,
};
use talaria_sources::is_followable_map_title;
use talaria_sources::load_seed_titles;
use talaria_sources::place_hint_from_title;
use talaria_sources::resolve_place_offline;
use talaria_sources::wdqs::{fetch_events_for_person, WdqsEvent};
use talaria_wikidata::WikidataClient;
use talaria_store::{
    apply_coords_to_event, find_active_person_event_by_occurrence, insert_claim,
    insert_claim_evidence, insert_person_event, insert_person_quote_evidence, update_entity_qid,
    upsert_raw_wikidata_document, upsert_raw_wikipedia_document, ClaimInsert, PersonEventInsert,
};
use uuid::Uuid;

use crate::llm::{self, LlmExtractItem};

pub async fn run_person_ingest(
    config: &AppConfig,
    subject: &str,
    qid: Option<&str>,
    wiki_lang: &str,
    max_documents: u32,
    seed_list: Option<&Path>,
) -> anyhow::Result<Value> {
    let (pool, entity_id) =
        crate::cli_helpers::open_db_for_subject(config, subject, "person").await?;
    let resolved_qid = resolve_person_qid(qid, subject, wiki_lang).await;
    if let Some(qid) = resolved_qid.as_deref() {
        let _ = update_entity_qid(&pool, entity_id, qid).await;
    }

    let follow_cap = follow_budget(max_documents);
    let mut facts_inserted = 0u32;
    let mut facts_reinforced = 0u32;
    let mut debates_inserted = 0u32;
    let mut dropped = 0u32;
    let mut wiki_pages = 0u32;
    let mut wdqs_events = 0u32;
    let mut seen_titles: HashSet<String> = HashSet::new();
    let mut follow_queue: Vec<String> = Vec::new();
    let mut primary_title = subject.to_string();

    for lang in wiki_langs(wiki_lang) {
        match fetch_wiki_extract(&lang, subject).await {
            Ok((title, text)) => {
                if !seen_titles.insert(title.to_lowercase()) {
                    continue;
                }
                if wiki_pages == 0 {
                    primary_title = title.clone();
                }
                wiki_pages += 1;
                let (ins, re, deb, drop) =
                    ingest_wiki_text(&pool, entity_id, subject, &lang, &title, &text).await?;
                facts_inserted += ins;
                facts_reinforced += re;
                debates_inserted += deb;
                dropped += drop;
            }
            Err(err) => tracing::warn!(lang, error = %err, "wikipedia extract failed"),
        }
    }

    if let Some(qid) = resolved_qid.as_deref() {
        match crate::lot_e::fetch_wikidata_subject_meta(qid, wiki_lang, Some(&pool)).await {
            Ok(meta) if !meta.statements_text.is_empty() => {
                let (doc, raw) = statements_to_raw_items(subject, &meta.statements_text);
                let wd_uri = format!("https://www.wikidata.org/wiki/{qid}");
                let wd_id =
                    upsert_raw_wikidata_document(&pool, &wd_uri, subject, &doc).await?;
                for item in accept_items(subject, &doc, raw) {
                    if item.lane == Lane::Fact {
                        match persist_fact(&pool, entity_id, subject, &item, wd_id, None, None)
                            .await?
                        {
                            PersistFact::Inserted => facts_inserted += 1,
                            PersistFact::Reinforced => facts_reinforced += 1,
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(err) => tracing::warn!(error = %err, "wikidata statements fetch failed"),
        }

        match fetch_events_for_person(qid).await {
            Ok(events) => {
                wdqs_events = events.len() as u32;
                let (wdqs_ins, wdqs_re) =
                    persist_wdqs_events(&pool, entity_id, subject, qid, &events).await?;
                facts_inserted += wdqs_ins;
                facts_reinforced += wdqs_re;
                follow_queue.extend(follow_titles_from_wdqs(&events, follow_cap));
            }
            Err(err) => tracing::warn!(error = %err, "wdqs participation harvest failed"),
        }
    }

    if let Some(path) = seed_list {
        match load_seed_titles(path) {
            Ok(seeds) => follow_queue.extend(seeds),
            Err(err) => tracing::debug!(error = %err, "seed list not loaded"),
        }
    }

    let extra = follow_cap.saturating_sub(wiki_pages);
    let mut followed = 0u32;
    for title in follow_queue {
        if followed >= extra {
            break;
        }
        if !should_pin_follow_title(&title) {
            continue;
        }
        if !seen_titles.insert(title.to_lowercase()) {
            continue;
        }
        match ingest_follow_page(
            &pool,
            entity_id,
            subject,
            wiki_lang,
            &title,
        )
        .await
        {
            Ok((ins, re)) => {
                if ins + re == 0 {
                    continue;
                }
                wiki_pages += 1;
                followed += 1;
                facts_inserted += ins;
                facts_reinforced += re;
            }
            Err(err) => tracing::debug!(title, error = %err, "follow wikipedia page skipped"),
        }
    }

    backfill_person_geocodes(&pool, entity_id).await?;

    Ok(json!({
        "lane": "explorer",
        "pipeline": "person",
        "subject": subject,
        "qid": resolved_qid,
        "entity_id": entity_id,
        "wikipedia_title": primary_title,
        "wiki_pages": wiki_pages,
        "wdqs_events": wdqs_events,
        "facts_inserted": facts_inserted,
        "facts_reinforced": facts_reinforced,
        "debates_inserted": debates_inserted,
        "llm_configured": llm::is_configured(),
        "dropped_chunks": dropped,
    }))
}

enum PersistFact {
    Inserted,
    Reinforced,
}

async fn persist_fact(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    subject: &str,
    item: &GroundedItem,
    raw_id: Uuid,
    coords: Option<(f64, f64)>,
    primary_object: Option<&str>,
) -> anyhow::Result<PersistFact> {
    let time = match item.year {
        Some(y) => TypedTime::Exact {
            year: y,
            month: None,
            day: None,
            surface: Some(y.to_string()),
        },
        None => TypedTime::Unknown { surface: None },
    };
    let occ = occurrence_key_for_event(
        subject,
        &item.event_type,
        &item.role,
        &time,
        item.place_surface.as_deref(),
        primary_object,
    );
    if let Some(existing) = find_active_person_event_by_occurrence(pool, entity_id, &occ).await? {
        insert_person_quote_evidence(
            pool,
            existing,
            &item.quoted_text,
            Some(raw_id),
            item.confidence,
        )
        .await?;
        return Ok(PersistFact::Reinforced);
    }

    let geo = if let Some((lat, lon)) = coords {
        Some(talaria_sources::PlaceResolution {
            label: item
                .place_surface
                .clone()
                .unwrap_or_else(|| primary_object.unwrap_or_default().to_string()),
            method: "wikidata_p625".into(),
            wikidata_qid: None,
            lat,
            lon,
            precision: "wikidata_p625".into(),
            uncertainty_radius_m: Some(5000.0),
            score: 0.9,
        })
    } else {
        geocode_place(item.place_surface.as_deref()).await
    };
    let map_eligible = geo.is_some();
    let year_label = item
        .year
        .map(|y| y.to_string())
        .unwrap_or_else(|| "undated".into());
    let place_label = item.place_surface.clone();
    let title = format!(
        "{subject} — {} ({year_label}){}",
        item.event_type,
        place_label
            .as_deref()
            .map(|p| format!(" @ {p}"))
            .unwrap_or_default()
    );
    let start_time = item
        .year
        .and_then(|y| Utc.with_ymd_and_hms(y, 1, 1, 0, 0, 0).single());
    let event_id = insert_person_event(
        pool,
        &PersonEventInsert {
            entity_id,
            event_type: item.event_type.clone(),
            epistemic_status: "attested".into(),
            title,
            summary: Some(item.summary.clone()),
            start_time,
            time_json: json!({
                "kind": "year",
                "year": item.year,
                "surface": item.year.map(|y| y.to_string()),
            }),
            place_label: place_label.clone(),
            lat: geo.as_ref().map(|g| g.lat),
            lon: geo.as_ref().map(|g| g.lon),
            confidence: item.confidence,
            map_eligible,
            fingerprint: occ.clone(),
            occurrence_key: occ,
            occurrence_stem: None,
            predicate: item.role.clone(),
        },
    )
    .await?;
    insert_person_quote_evidence(
        pool,
        event_id,
        &item.quoted_text,
        Some(raw_id),
        item.confidence,
    )
    .await?;
    Ok(PersistFact::Inserted)
}

async fn persist_debate(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    item: &GroundedItem,
    uri: &str,
) -> anyhow::Result<()> {
    let claim_id = insert_claim(
        pool,
        &ClaimInsert {
            entity_id,
            claim_kind: "controversy".into(),
            text: item.summary.clone(),
            epistemic_status: "theory".into(),
            relation_to_subject: "historiography".into(),
            event_time: None,
            place_label: item.place_surface.clone(),
            confidence: item.confidence,
            canonical_event_id: None,
            debate_type: Some("controversy".into()),
            evidence_layer: Some("llm_grounded".into()),
        },
    )
    .await?;
    insert_claim_evidence(
        pool,
        claim_id,
        "wikipedia",
        Some(uri),
        Some(item.quoted_text.as_str()),
        None,
        item.confidence,
    )
    .await?;
    Ok(())
}

fn wiki_langs(requested: &str) -> Vec<String> {
    let mut langs = vec![requested.trim().to_ascii_lowercase()];
    for extra in ["en", "fr"] {
        if !langs.iter().any(|l| l == extra) {
            langs.push(extra.to_string());
        }
    }
    langs
}

async fn resolve_person_qid(
    explicit: Option<&str>,
    subject: &str,
    lang: &str,
) -> Option<String> {
    if let Some(qid) = explicit
        .map(str::trim)
        .filter(|q| q.starts_with('Q') && q.len() > 1)
    {
        return Some(qid.to_string());
    }
    let client = WikidataClient::new().ok()?;
    if let Ok(Some(qid)) = client.search_entity(subject, lang).await {
        return Some(qid);
    }
    if lang != "en" {
        return client.search_entity(subject, "en").await.ok().flatten();
    }
    None
}

fn year_from_wdqs_date(date: &str) -> Option<i32> {
    let y: i32 = date.get(..4)?.parse().ok()?;
    (y != 0).then_some(y)
}

fn coords_for_wdqs(ev: &WdqsEvent) -> Option<(f64, f64)> {
    match (ev.lat, ev.lon) {
        (Some(lat), Some(lon)) if lat.abs() <= 90.0 && lon.abs() <= 180.0 => Some((lat, lon)),
        _ => None,
    }
}

fn wdqs_event_to_extract(subject: &str, ev: &WdqsEvent) -> (String, RawExtractItem, Option<(f64, f64)>) {
    let year = year_from_wdqs_date(&ev.date);
    let place = ev
        .place_label
        .clone()
        .or_else(|| place_hint_from_title(&ev.label));
    let quote = format!(
        "{subject} | {} | {} | {} | {}",
        ev.event_type,
        ev.label,
        year.map(|y| y.to_string()).unwrap_or_default(),
        place.as_deref().unwrap_or("")
    );
    let item = RawExtractItem {
        lane: "fact".into(),
        event_type: if ev.event_type.is_empty() {
            "historical_fact".into()
        } else {
            ev.event_type.clone()
        },
        role: "direct".into(),
        year,
        place_surface: place,
        summary: ev.label.clone(),
        quoted_text: quote.clone(),
        confidence: 0.95,
    };
    (quote, item, coords_for_wdqs(ev))
}

fn follow_titles_from_wdqs(events: &[WdqsEvent], cap: u32) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for ev in events {
        if out.len() as u32 >= cap {
            break;
        }
        if !is_followable_map_title(&ev.label) {
            continue;
        }
        let key = ev.label.to_lowercase();
        if seen.insert(key) {
            out.push(ev.label.clone());
        }
    }
    out
}

fn follow_budget(max_documents: u32) -> u32 {
    max_documents.max(8).min(400)
}

fn fold_name(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'à' | 'á' | 'â' | 'ä' | 'À' | 'Á' | 'Â' => 'a',
            'î' | 'ï' | 'Î' | 'Ï' => 'i',
            'ô' | 'ö' | 'Ô' => 'o',
            'ù' | 'ú' | 'û' | 'Ù' => 'u',
            'ç' | 'Ç' => 'c',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

fn subject_aliases(subject: &str) -> Vec<String> {
    let mut out = vec![fold_name(subject)];
    for part in subject.split_whitespace() {
        let folded = fold_name(part);
        if folded.len() >= 4 && !out.iter().any(|a| a == &folded) {
            out.push(folded);
        }
    }
    out
}

fn subject_mentioned(text: &str, subject: &str) -> bool {
    let hay = fold_name(text);
    subject_aliases(subject).iter().any(|a| hay.contains(a))
}

fn is_overview_title(title: &str) -> bool {
    let l = title.to_lowercase();
    l.starts_with("list of")
        || l.starts_with("liste des")
        || l.starts_with("liste de")
        || l.starts_with("timeline of")
        || l.starts_with("military career")
        || l.starts_with("early life")
        || l.starts_with("scientific career")
        || l.ends_with(" wars")
        || l.ends_with(" war")
}

fn should_pin_follow_title(title: &str) -> bool {
    is_followable_map_title(title) && !is_overview_title(title)
}

fn event_type_from_title(title: &str) -> String {
    let l = title.to_lowercase();
    if l.contains("battle") || l.contains("bataille") || l.contains("siege") || l.contains("siège")
    {
        "battle".into()
    } else if l.contains("treaty") || l.contains("traité") || l.contains("treaties") {
        "diplomatic".into()
    } else if l.contains("palace") || l.contains("château") || l.contains("chateau") {
        "residence".into()
    } else {
        "historical_fact".into()
    }
}

fn year_from_text(text: &str) -> Option<i32> {
    talaria_sources::first_year_in_window(text, 1000, 2099)?.parse().ok()
}

fn follow_page_to_extract(
    subject: &str,
    title: &str,
    extract: &str,
    coords: Option<(f64, f64)>,
) -> Option<(String, RawExtractItem, Option<(f64, f64)>)> {
    if !subject_mentioned(extract, subject) {
        return None;
    }
    let event_type = event_type_from_title(title);
    let year = year_from_text(extract).or_else(|| year_from_text(title));
    let place = place_hint_from_title(title);
    let quote = format!(
        "{subject} | {event_type} | {title} | {} | {}",
        year.map(|y| y.to_string()).unwrap_or_default(),
        place.as_deref().unwrap_or("")
    );
    let doc = format!("{quote}\n{extract}");
    let item = RawExtractItem {
        lane: "fact".into(),
        event_type,
        role: "direct".into(),
        year,
        place_surface: place,
        summary: title.to_string(),
        quoted_text: quote,
        confidence: 0.88,
    };
    Some((doc, item, coords))
}

async fn ingest_follow_page(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    subject: &str,
    lang: &str,
    title: &str,
) -> anyhow::Result<(u32, u32)> {
    let (resolved, text, wiki_xy) = fetch_wiki_page(lang, title).await?;
    let Some((doc, raw, mut xy)) = follow_page_to_extract(subject, &resolved, &text, wiki_xy) else {
        return Ok((0, 0));
    };
    if xy.is_none() {
        xy = geocode_place(raw.place_surface.as_deref())
            .await
            .map(|g| (g.lat, g.lon));
    }
    let uri = format!(
        "https://{lang}.wikipedia.org/wiki/{}",
        resolved.replace(' ', "_")
    );
    let raw_id = upsert_raw_wikipedia_document(pool, &uri, &resolved, lang, &doc).await?;
    let mut inserted = 0u32;
    let mut reinforced = 0u32;
    for item in accept_items(subject, &doc, [raw]) {
        if item.lane != Lane::Fact {
            continue;
        }
        match persist_fact(
            pool,
            entity_id,
            subject,
            &item,
            raw_id,
            xy,
            Some(resolved.as_str()),
        )
        .await?
        {
            PersistFact::Inserted => inserted += 1,
            PersistFact::Reinforced => reinforced += 1,
        }
    }
    Ok((inserted, reinforced))
}

async fn persist_wdqs_events(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    subject: &str,
    qid: &str,
    events: &[WdqsEvent],
) -> anyhow::Result<(u32, u32)> {
    if events.is_empty() {
        return Ok((0, 0));
    }
    let mut lines = Vec::new();
    for ev in events {
        let (quote, _, _) = wdqs_event_to_extract(subject, ev);
        lines.push(quote);
    }
    let doc = lines.join("\n");
    let uri = format!("https://www.wikidata.org/wiki/{qid}#participation");
    let raw_id = upsert_raw_wikidata_document(pool, &uri, subject, &doc).await?;
    let mut inserted = 0u32;
    let mut reinforced = 0u32;
    for ev in events {
        let (quote, raw, xy) = wdqs_event_to_extract(subject, ev);
        let _ = quote;
        let Ok(item) = talaria_quality::validate_item(&raw, &doc, subject) else {
            continue;
        };
        if item.lane != Lane::Fact {
            continue;
        }
        match persist_fact(
            pool,
            entity_id,
            subject,
            &item,
            raw_id,
            xy,
            Some(ev.label.as_str()),
        )
        .await?
        {
            PersistFact::Inserted => inserted += 1,
            PersistFact::Reinforced => reinforced += 1,
        }
    }
    Ok((inserted, reinforced))
}

async fn ingest_wiki_text(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    subject: &str,
    lang: &str,
    title: &str,
    text: &str,
) -> anyhow::Result<(u32, u32, u32, u32)> {
    let uri = format!(
        "https://{lang}.wikipedia.org/wiki/{}",
        title.replace(' ', "_")
    );
    let raw_id = upsert_raw_wikipedia_document(pool, &uri, title, lang, text).await?;
    let mut inserted = 0u32;
    let mut reinforced = 0u32;
    let mut debates = 0u32;
    let mut dropped = 0u32;
    for chunk in split_chunks(text, 3500) {
        let extracted = if llm::is_configured() {
            match llm::extract_chunk(subject, title, &chunk).await {
                Ok(items) => items,
                Err(err) => {
                    tracing::warn!(title, error = %err, "llm extract failed for chunk");
                    dropped += 1;
                    continue;
                }
            }
        } else {
            Vec::new()
        };
        let raw = extracted.into_iter().map(LlmExtractItem::into_raw);
        for item in accept_items(subject, &chunk, raw) {
            match item.lane {
                Lane::Debate => {
                    persist_debate(pool, entity_id, &item, &uri).await?;
                    debates += 1;
                }
                Lane::Fact => {
                    match persist_fact(pool, entity_id, subject, &item, raw_id, None, None).await? {
                        PersistFact::Inserted => inserted += 1,
                        PersistFact::Reinforced => reinforced += 1,
                    }
                }
            }
        }
    }
    Ok((inserted, reinforced, debates, dropped))
}

fn statements_to_raw_items(subject: &str, statements: &str) -> (String, Vec<RawExtractItem>) {
    let mut lines = Vec::new();
    let mut items = Vec::new();
    for line in statements.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 5 || parts[0] != "STATEMENT" {
            continue;
        }
        let event_type = parts[1].trim();
        let pred = parts[2].trim();
        let year = parts[3].trim().parse::<i32>().ok();
        let place = parts[4].trim();
        if event_type.is_empty() {
            continue;
        }
        let quote = format!(
            "{subject} | {event_type} | {pred} | {} | {place}",
            parts[3].trim()
        );
        lines.push(quote.clone());
        items.push(RawExtractItem {
            lane: "fact".into(),
            event_type: event_type.into(),
            role: "direct".into(),
            year,
            place_surface: if place.is_empty() {
                None
            } else {
                Some(place.to_string())
            },
            summary: format!("{pred} {place}").trim().to_string(),
            quoted_text: quote,
            confidence: 0.92,
        });
    }
    (lines.join("\n"), items)
}

async fn geocode_place(label: Option<&str>) -> Option<talaria_sources::PlaceResolution> {
    let label = label.map(str::trim).filter(|s| !s.is_empty())?;
    if let Some(res) = resolve_place_offline(label) {
        return Some(res);
    }
    let hit = crate::lot_e::resolve_label_coords(label).await?;
    Some(talaria_sources::PlaceResolution {
        label: label.to_string(),
        method: "wikidata_p625".into(),
        wikidata_qid: None,
        lat: hit.lat,
        lon: hit.lon,
        precision: hit.precision,
        uncertainty_radius_m: hit.uncertainty,
        score: 0.7,
    })
}

async fn backfill_person_geocodes(pool: &sqlx::PgPool, entity_id: Uuid) -> anyhow::Result<()> {
    let rows: Vec<(Uuid, Option<String>)> = sqlx::query_as(
        r#"
        SELECT id, place_label FROM canonical_events
        WHERE entity_id = $1 AND pipeline = 'person' AND is_active
          AND place_label IS NOT NULL AND btrim(place_label) <> ''
        "#,
    )
    .bind(entity_id)
    .fetch_all(pool)
    .await?;
    for (id, label) in rows {
        let Some(label) = label else { continue };
        if let Some(geo) = geocode_place(Some(&label)).await {
            apply_coords_to_event(pool, id, geo.lat, geo.lon).await?;
        }
    }
    Ok(())
}

fn split_chunks(text: &str, max: usize) -> Vec<String> {
    if text.len() <= max {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    let mut buf = String::new();
    for para in text.split("\n\n") {
        if buf.len() + para.len() + 2 > max && !buf.is_empty() {
            out.push(std::mem::take(&mut buf));
        }
        if !buf.is_empty() {
            buf.push_str("\n\n");
        }
        buf.push_str(para);
    }
    if !buf.is_empty() {
        out.push(buf);
    }
    out
}

async fn fetch_wiki_extract(lang: &str, title: &str) -> anyhow::Result<(String, String)> {
    let client = reqwest::Client::builder()
        .user_agent("TalariaEngine/0.1 (person-ingest)")
        .timeout(std::time::Duration::from_secs(45))
        .build()?;
    let url = format!("https://{lang}.wikipedia.org/w/api.php");
    let body: Value = client
        .get(&url)
        .query(&[
            ("action", "query"),
            ("prop", "extracts"),
            ("explaintext", "1"),
            ("exlimit", "1"),
            ("titles", title),
            ("format", "json"),
            ("redirects", "1"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let pages = body
        .pointer("/query/pages")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow::anyhow!("wikipedia extract missing pages"))?;
    let page = pages
        .values()
        .next()
        .ok_or_else(|| anyhow::anyhow!("wikipedia extract empty"))?;
    if page.get("missing").is_some() {
        anyhow::bail!("wikipedia page missing: {title}");
    }
    let resolved = page
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or(title)
        .to_string();
    let extract = page
        .get("extract")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if extract.trim().is_empty() {
        anyhow::bail!("wikipedia extract empty for {title}");
    }
    Ok((resolved, extract))
}

async fn fetch_wiki_page(
    lang: &str,
    title: &str,
) -> anyhow::Result<(String, String, Option<(f64, f64)>)> {
    let client = reqwest::Client::builder()
        .user_agent("TalariaEngine/0.1 (person-ingest)")
        .timeout(std::time::Duration::from_secs(45))
        .build()?;
    let url = format!("https://{lang}.wikipedia.org/w/api.php");
    let body: Value = client
        .get(&url)
        .query(&[
            ("action", "query"),
            ("prop", "extracts|coordinates"),
            ("explaintext", "1"),
            ("exlimit", "1"),
            ("colimit", "1"),
            ("titles", title),
            ("format", "json"),
            ("redirects", "1"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let pages = body
        .pointer("/query/pages")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow::anyhow!("wikipedia page missing pages"))?;
    let page = pages
        .values()
        .next()
        .ok_or_else(|| anyhow::anyhow!("wikipedia page empty"))?;
    if page.get("missing").is_some() {
        anyhow::bail!("wikipedia page missing: {title}");
    }
    let resolved = page
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or(title)
        .to_string();
    let extract = page
        .get("extract")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if extract.trim().is_empty() {
        anyhow::bail!("wikipedia extract empty for {title}");
    }
    let coords = page
        .get("coordinates")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|c| {
            Some((c.get("lat")?.as_f64()?, c.get("lon")?.as_f64()?))
        });
    Ok((resolved, extract, coords))
}

#[cfg(test)]
mod tests {
    use super::*;
    use talaria_quality::RawExtractItem;

    #[test]
    fn schrodinger_chunk_yields_no_curie_facts() {
        let doc = "On 6 April 1920, Schrödinger married Annemarie Bertel in Vienna.";
        let raw = [RawExtractItem {
            lane: "fact".into(),
            event_type: "marriage".into(),
            role: "direct".into(),
            year: Some(1920),
            place_surface: Some("Vienna".into()),
            summary: "marriage".into(),
            quoted_text: doc.into(),
            confidence: 0.9,
        }];
        assert!(accept_items("Marie Curie", doc, raw).is_empty());
    }

    #[test]
    fn wikidata_birth_statement_becomes_fact() {
        let statements = "STATEMENT\tbirth\tborn_in\t1867\tWarsaw";
        let (doc, raw) = statements_to_raw_items("Marie Curie", statements);
        let got = accept_items("Marie Curie", &doc, raw);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].event_type, "birth");
        assert_eq!(got[0].place_surface.as_deref(), Some("Warsaw"));
    }

    #[test]
    fn chunks_split_long_text() {
        let text = "aaaa\n\nbbbb\n\ncccc";
        assert_eq!(split_chunks(text, 8).len(), 3);
    }

    #[test]
    fn year_from_wdqs_iso_date() {
        assert_eq!(year_from_wdqs_date("1805-12-02"), Some(1805));
        assert_eq!(year_from_wdqs_date(""), None);
    }

    #[test]
    fn wdqs_battle_with_coords_is_a_grounded_napoleon_fact() {
        let ev = talaria_sources::wdqs::WdqsEvent {
            event_qid: "Q179250".into(),
            label: "Battle of Austerlitz".into(),
            date: "1805-12-02".into(),
            place_qid: None,
            place_label: Some("Austerlitz".into()),
            event_type: "battle".into(),
            lat: Some(49.1281),
            lon: Some(16.7622),
        };
        let (quote, item, coords) = wdqs_event_to_extract("Napoleon", &ev);
        assert_eq!(coords, Some((49.1281, 16.7622)));
        assert_eq!(item.event_type, "battle");
        assert_eq!(item.place_surface.as_deref(), Some("Austerlitz"));
        let got = accept_items("Napoleon", &quote, [item]);
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn follow_budget_uses_requested_document_cap() {
        assert_eq!(follow_budget(400), 400);
        assert_eq!(follow_budget(0), 8);
        assert_eq!(follow_budget(9_000), 400);
        assert!(follow_budget(400) > 80);
    }

    #[test]
    fn battle_pages_are_pinned_lists_are_not() {
        assert!(should_pin_follow_title("Battle of Waterloo"));
        assert!(should_pin_follow_title("Bataille d'Austerlitz"));
        assert!(should_pin_follow_title("Treaty of Tilsit"));
        assert!(!should_pin_follow_title(
            "List of battles of the Napoleonic Wars"
        ));
        assert!(!should_pin_follow_title("Napoleonic Wars"));
        assert!(!should_pin_follow_title("Military career of Napoleon"));
    }

    #[test]
    fn napoleon_seed_list_has_dozens_of_pin_titles() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/seeds/napoleon_wiki_titles.txt");
        let titles = talaria_sources::load_seed_titles(&path).expect("napoleon seed list");
        let pins = titles
            .iter()
            .filter(|t| should_pin_follow_title(t))
            .count();
        assert!(
            pins >= 80,
            "expected a dense Napoleon battle/treaty seed list, got {pins}"
        );
    }

    #[test]
    fn follow_page_mentioning_napoleon_is_a_grounded_map_fact() {
        let extract = "The Battle of Waterloo was fought on Sunday 18 June 1815 near Waterloo. Napoleon's French army was defeated by the Duke of Wellington.";
        let (doc, item, coords) = follow_page_to_extract(
            "Napoleon",
            "Battle of Waterloo",
            extract,
            Some((50.680, 4.412)),
        )
        .expect("pin");
        assert_eq!(coords, Some((50.680, 4.412)));
        assert_eq!(item.event_type, "battle");
        assert_eq!(item.place_surface.as_deref(), Some("Waterloo"));
        let got = accept_items("Napoleon", &doc, [item]);
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn follow_page_without_subject_mention_is_dropped() {
        let extract = "Wellington commanded the allied army on 18 June 1815 near Waterloo.";
        assert!(follow_page_to_extract(
            "Napoleon",
            "Battle of Waterloo",
            extract,
            Some((50.680, 4.412)),
        )
        .is_none());
    }
}
