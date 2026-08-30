// crates/talaria-api/src/person_ingest/mod.rs
//! One ingest: dump Wikipedia → grounded LLM → pipeline='person' facts + Agora debates.

mod collect;
mod extract;
mod gating;
mod grounding;
mod persist;
mod resolve;
mod typing;

use std::collections::HashSet;
use std::path::Path;

use serde_json::{json, Value};
use talaria_core::AppConfig;
use talaria_quality::{GateContext, Lane};
use talaria_sources::has_military_signal;
use talaria_sources::load_seed_titles;
use talaria_sources::wdqs::fetch_events_for_person;
use talaria_store::{
    connect, run_migrations, upsert_person_by_qid, upsert_raw_wikidata_document,
    upsert_raw_wikipedia_document,
};
use uuid::Uuid;

use crate::llm;
use persist::{PersistMeta, PersistOutcome};

pub async fn run_person_ingest(
    config: &AppConfig,
    subject: &str,
    qid: Option<&str>,
    wiki_lang: &str,
    max_documents: u32,
    seed_list: Option<&Path>,
) -> anyhow::Result<Value> {
    let resolved_qid = resolve::require_person_qid(qid, subject, wiki_lang).await?;
    let pool = connect(config).await?;
    run_migrations(&pool).await?;

    let mut facts_inserted = 0u32;
    let mut facts_reinforced = 0u32;
    let mut debates_inserted = 0u32;
    let mut dropped = 0u32;
    let mut wiki_pages = 0u32;
    let mut wdqs_events = 0u32;
    let mut seen_titles: HashSet<String> = HashSet::new();
    let mut follow_queue: Vec<String> = Vec::new();
    let mut primary_title = subject.to_string();
    let mut aliases = collect::subject_aliases(subject);
    let follow_cap = collect::follow_budget(max_documents);

    let wd_meta = match crate::lot_e::fetch_wikidata_subject_meta(&resolved_qid, wiki_lang, Some(&pool))
        .await
    {
        Ok(meta) => Some(meta),
        Err(err) => {
            tracing::warn!(error = %err, "wikidata statements fetch failed");
            None
        }
    };
    let wiki_title = wd_meta
        .as_ref()
        .and_then(|m| m.wiki_title.clone())
        .unwrap_or_else(|| subject.to_string());
    if wiki_title != subject {
        for extra in collect::subject_aliases(&wiki_title) {
            if !aliases.iter().any(|a| a == &extra) {
                aliases.push(extra);
            }
        }
    }
    let military_subject = wd_meta
        .as_ref()
        .map(|m| has_military_signal(&m.occupations, Some(subject)))
        .unwrap_or(false);
    let wd_label = wiki_title.clone();
    let entity_id = upsert_person_by_qid(
        &pool,
        &resolved_qid,
        &wd_label,
        wiki_lang,
        &wiki_title,
        subject,
    )
    .await?;

    let mut ctx = GateContext {
        subject_birth_year: wd_meta.as_ref().and_then(|m| m.birth_year),
        subject_death_year: wd_meta.as_ref().and_then(|m| m.death_year),
        ..Default::default()
    };

    if let Some(meta) = wd_meta.as_ref() {
        if !meta.statements_text.is_empty() {
            let (doc, raw) = extract::statements_to_raw_items(subject, &meta.statements_text);
            let wd_uri = format!("https://www.wikidata.org/wiki/{resolved_qid}");
            let wd_id = upsert_raw_wikidata_document(&pool, &wd_uri, subject, &doc).await?;
            let locator = grounding::wikidata_locator(&resolved_qid);
            for item in grounding::ground_structured(subject, &doc, raw, Some(&resolved_qid)) {
                if item.lane != Lane::Fact {
                    continue;
                }
                if item.event_type != "birth" && item.event_type != "death" {
                    continue;
                }
                tally(
                    persist::persist_fact_item(
                        &pool,
                        entity_id,
                        subject,
                        &item,
                        &mut ctx,
                        PersistMeta {
                            raw_document_id: wd_id,
                            coords: None,
                            primary_object: None,
                            source_locator: &locator,
                            page_title: subject,
                            from_followed_page: false,
                            structured_source: true,
                            military_subject,
                            aliases: &aliases,
                        },
                    )
                    .await?,
                    &mut facts_inserted,
                    &mut facts_reinforced,
                );
            }
        }
    }

    for lang in collect::wiki_langs(wiki_lang) {
        match collect::fetch_wiki_extract(&lang, subject).await {
            Ok((title, text, links)) => {
                if !seen_titles.insert(title.to_lowercase()) {
                    continue;
                }
                if wiki_pages == 0 {
                    primary_title = title.clone();
                }
                wiki_pages += 1;
                follow_queue.extend(collect::follow_titles_from_page_links(&links, follow_cap));
                let (ins, re, deb, drop) = ingest_wiki_text(
                    &pool,
                    entity_id,
                    subject,
                    &lang,
                    &title,
                    &text,
                    &mut ctx,
                    &aliases,
                    military_subject,
                )
                .await?;
                facts_inserted += ins;
                facts_reinforced += re;
                debates_inserted += deb;
                dropped += drop;
            }
            Err(err) => tracing::warn!(lang, error = %err, "wikipedia extract failed"),
        }
    }

    if let Some(meta) = wd_meta.as_ref() {
        if !meta.statements_text.is_empty() {
            let (doc, raw) = extract::statements_to_raw_items(subject, &meta.statements_text);
            let wd_uri = format!("https://www.wikidata.org/wiki/{resolved_qid}");
            let wd_id = upsert_raw_wikidata_document(&pool, &wd_uri, subject, &doc).await?;
            let locator = grounding::wikidata_locator(&resolved_qid);
            for item in grounding::ground_structured(subject, &doc, raw, Some(&resolved_qid)) {
                if item.lane != Lane::Fact {
                    continue;
                }
                if item.event_type == "birth" || item.event_type == "death" {
                    continue;
                }
                tally(
                    persist::persist_fact_item(
                        &pool,
                        entity_id,
                        subject,
                        &item,
                        &mut ctx,
                        PersistMeta {
                            raw_document_id: wd_id,
                            coords: None,
                            primary_object: None,
                            source_locator: &locator,
                            page_title: subject,
                            from_followed_page: false,
                            structured_source: true,
                            military_subject,
                            aliases: &aliases,
                        },
                    )
                    .await?,
                    &mut facts_inserted,
                    &mut facts_reinforced,
                );
            }
        }
    }

    match fetch_events_for_person(&resolved_qid).await {
        Ok(events) => {
            wdqs_events = events.len() as u32;
            let (wdqs_ins, wdqs_re) = persist_wdqs_events(
                &pool,
                entity_id,
                subject,
                &resolved_qid,
                &events,
                &mut ctx,
                &aliases,
                military_subject,
            )
            .await?;
            facts_inserted += wdqs_ins;
            facts_reinforced += wdqs_re;
            follow_queue.extend(collect::follow_titles_from_wdqs(&events, follow_cap));
        }
        Err(err) => tracing::warn!(error = %err, "wdqs participation harvest failed"),
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
        if !collect::should_pin_follow_title(&title) {
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
            &mut ctx,
            &aliases,
            military_subject,
        )
        .await
        {
            Ok((ins, re)) => {
                followed += 1;
                if ins + re > 0 {
                    wiki_pages += 1;
                }
                facts_inserted += ins;
                facts_reinforced += re;
            }
            Err(err) => tracing::debug!(title, error = %err, "follow wikipedia page skipped"),
        }
    }

    typing::backfill_person_geocodes(&pool, entity_id).await?;

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
        "llm_model": llm::model(),
        "dropped_chunks": dropped,
    }))
}

fn tally(outcome: PersistOutcome, inserted: &mut u32, reinforced: &mut u32) {
    match outcome {
        PersistOutcome::Canonical {
            inserted: true, ..
        } => *inserted += 1,
        PersistOutcome::Canonical {
            inserted: false, ..
        } => *reinforced += 1,
        PersistOutcome::CandidateOnly { .. } => {}
    }
}

async fn ingest_wiki_text(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    subject: &str,
    lang: &str,
    title: &str,
    text: &str,
    ctx: &mut GateContext,
    aliases: &[String],
    military_subject: bool,
) -> anyhow::Result<(u32, u32, u32, u32)> {
    let uri = format!(
        "https://{lang}.wikipedia.org/wiki/{}",
        title.replace(' ', "_")
    );
    let raw_id = upsert_raw_wikipedia_document(pool, &uri, title, lang, text).await?;
    let locator = grounding::text_span_locator(&uri, title);
    let mut inserted = 0u32;
    let mut reinforced = 0u32;
    let mut debates = 0u32;
    let mut dropped = 0u32;
    let rules = extract::extract_wiki_rules(subject, title, text, ctx.subject_death_year);
    for item in grounding::ground_prose(subject, text, rules) {
        if item.lane != Lane::Fact {
            continue;
        }
        tally(
            persist::persist_fact_item(
                pool,
                entity_id,
                subject,
                &item,
                ctx,
                PersistMeta {
                    raw_document_id: raw_id,
                    coords: None,
                    primary_object: None,
                    source_locator: &locator,
                    page_title: title,
                    from_followed_page: false,
                    structured_source: false,
                    military_subject,
                    aliases,
                },
            )
            .await?,
            &mut inserted,
            &mut reinforced,
        );
    }
    if inserted + reinforced > 0 {
        return Ok((inserted, reinforced, debates, dropped));
    }
    for chunk in extract::split_chunks(text, 3500) {
        let extracted = if llm::is_configured() {
            match extract::extract_prose_chunk(subject, title, &chunk).await {
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
        for item in grounding::ground_prose(subject, &chunk, extracted) {
            match item.lane {
                Lane::Debate => {
                    persist::persist_debate(pool, entity_id, &item, &uri).await?;
                    debates += 1;
                }
                Lane::Fact => {
                    tally(
                        persist::persist_fact_item(
                            pool,
                            entity_id,
                            subject,
                            &item,
                            ctx,
                            PersistMeta {
                                raw_document_id: raw_id,
                                coords: None,
                                primary_object: None,
                                source_locator: &locator,
                                page_title: title,
                                from_followed_page: false,
                                structured_source: false,
                                military_subject,
                                aliases,
                            },
                        )
                        .await?,
                        &mut inserted,
                        &mut reinforced,
                    );
                }
            }
        }
    }
    Ok((inserted, reinforced, debates, dropped))
}

async fn ingest_follow_page(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    subject: &str,
    lang: &str,
    title: &str,
    ctx: &mut GateContext,
    aliases: &[String],
    military_subject: bool,
) -> anyhow::Result<(u32, u32)> {
    let (resolved, text, wiki_xy) = collect::fetch_wiki_page(lang, title).await?;
    let Some((doc, raw, mut xy)) = extract::follow_page_to_extract(
        subject,
        &resolved,
        &text,
        wiki_xy,
        military_subject,
    )
    else {
        return Ok((0, 0));
    };
    if xy.is_none() {
        xy = typing::geocode_place(raw.place_surface.as_deref())
            .await
            .map(|g| (g.lat, g.lon));
    }
    let uri = format!(
        "https://{lang}.wikipedia.org/wiki/{}",
        resolved.replace(' ', "_")
    );
    let raw_id = upsert_raw_wikipedia_document(pool, &uri, &resolved, lang, &doc).await?;
    let locator = grounding::text_span_locator(&uri, &resolved);
    let mut inserted = 0u32;
    let mut reinforced = 0u32;
    for item in grounding::ground_prose(subject, &doc, [raw]) {
        if item.lane != Lane::Fact {
            continue;
        }
        tally(
            persist::persist_fact_item(
                pool,
                entity_id,
                subject,
                &item,
                ctx,
                PersistMeta {
                    raw_document_id: raw_id,
                    coords: xy,
                    primary_object: Some(resolved.as_str()),
                    source_locator: &locator,
                    page_title: resolved.as_str(),
                    from_followed_page: true,
                    structured_source: false,
                    military_subject,
                    aliases,
                },
            )
            .await?,
            &mut inserted,
            &mut reinforced,
        );
    }
    Ok((inserted, reinforced))
}

async fn persist_wdqs_events(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    subject: &str,
    qid: &str,
    events: &[talaria_sources::wdqs::WdqsEvent],
    ctx: &mut GateContext,
    aliases: &[String],
    military_subject: bool,
) -> anyhow::Result<(u32, u32)> {
    if events.is_empty() {
        return Ok((0, 0));
    }
    let mut lines = Vec::new();
    for ev in events {
        let (quote, _, _) = extract::wdqs_event_to_extract(subject, ev);
        lines.push(quote);
    }
    let doc = lines.join("\n");
    let uri = format!("https://www.wikidata.org/wiki/{qid}#participation");
    let raw_id = upsert_raw_wikidata_document(pool, &uri, subject, &doc).await?;
    let mut inserted = 0u32;
    let mut reinforced = 0u32;
    for ev in events {
        let (_quote, raw, xy) = extract::wdqs_event_to_extract(subject, ev);
        let locator = grounding::wikidata_locator(&ev.event_qid);
        let items = grounding::ground_structured(subject, &doc, [raw], Some(&ev.event_qid));
        for item in items {
            if item.lane != Lane::Fact {
                continue;
            }
            tally(
                persist::persist_fact_item(
                    pool,
                    entity_id,
                    subject,
                    &item,
                    ctx,
                    PersistMeta {
                        raw_document_id: raw_id,
                        coords: xy,
                        primary_object: Some(ev.label.as_str()),
                        source_locator: &locator,
                        page_title: &ev.label,
                        from_followed_page: false,
                        structured_source: true,
                        military_subject,
                        aliases,
                    },
                )
                .await?,
                &mut inserted,
                &mut reinforced,
            );
        }
    }
    Ok((inserted, reinforced))
}
