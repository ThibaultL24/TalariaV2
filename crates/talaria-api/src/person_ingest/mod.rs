// crates/talaria-api/src/person_ingest/mod.rs
//! One ingest: dump Wikipedia → grounded LLM → pipeline='person' facts + Agora debates.

mod collect;
pub mod corpus_evidence;
pub mod crawl_queue;
mod extract;
mod gating;
mod grounding;
mod persist;
mod resolve;
pub(crate) mod typing;

use std::path::Path;

use serde_json::{json, Value};
use talaria_core::AppConfig;
use talaria_quality::{GateContext, Lane};
use talaria_sources::has_military_signal;
use talaria_sources::load_seed_titles;
use talaria_sources::wdqs::fetch_events_for_person;
use talaria_store::{
    connect, quality_lifespan_years, run_migrations, update_entity_authority_ids,
    upsert_person_by_qid, upsert_raw_wikidata_document, upsert_raw_wikipedia_document,
};
use uuid::Uuid;

use crate::llm;
use crate::routes::ingest::ProgressHandle;
use crawl_queue::{
    crawl_items_from_page_links, crawl_items_from_seed_list, crawl_items_from_wdqs,
    CrawlPriority, PriorityCrawlQueue,
};
use persist::{PersistMeta, PersistOutcome};

/// How many model calls one page may make. Rules still run on the whole page.
const MAX_LLM_CHUNKS_PER_PAGE: usize = 4;

/// Run person ingest with optional progress handle for UI updates.
/// Uses prioritized crawl queue to process core subject pages first, then expand.
pub async fn run_person_ingest_progressive(
    config: &AppConfig,
    subject: &str,
    qid: Option<&str>,
    wiki_lang: &str,
    max_documents: u32,
    seed_list: Option<&Path>,
    progress: Option<ProgressHandle>,
) -> anyhow::Result<Value> {
    run_person_ingest_inner(config, subject, qid, wiki_lang, max_documents, seed_list, progress)
        .await
}

/// Run person ingest without progress tracking (backward compatibility).
pub async fn run_person_ingest(
    config: &AppConfig,
    subject: &str,
    qid: Option<&str>,
    wiki_lang: &str,
    max_documents: u32,
    seed_list: Option<&Path>,
) -> anyhow::Result<Value> {
    run_person_ingest_inner(config, subject, qid, wiki_lang, max_documents, seed_list, None).await
}

/// Internal implementation with optional progress tracking and prioritized crawl queue.
async fn run_person_ingest_inner(
    config: &AppConfig,
    subject: &str,
    qid: Option<&str>,
    wiki_lang: &str,
    max_documents: u32,
    seed_list: Option<&Path>,
    progress: Option<ProgressHandle>,
) -> anyhow::Result<Value> {
    if let Some(ref p) = progress {
        p.set_phase("resolving").await;
    }

    let resolved_qid = resolve::require_person_qid(qid, subject, wiki_lang).await?;
    let pool = connect(config).await?;
    run_migrations(&pool).await?;

    let mut facts_inserted = 0u32;
    let mut facts_reinforced = 0u32;
    let mut debates_inserted = 0u32;
    let mut dropped = 0u32;
    let mut wiki_pages = 0u32;
    let mut wdqs_events = 0u32;
    let mut crawl_queue = PriorityCrawlQueue::new();
    let mut primary_title = subject.to_string();
    let mut aliases = collect::subject_aliases(subject);
    let follow_cap = collect::follow_budget(max_documents);

    if let Some(ref p) = progress {
        p.set_phase("wikidata").await;
    }

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

    // Notify progress handle of resolved entity_id
    if let Some(ref p) = progress {
        p.set_entity_id(entity_id).await;
    }

    // Persist authority bundle (P268 BnF, P214 VIAF, P213 ISNI, P269 IdRef) when available.
    if let Some(meta) = wd_meta.as_ref() {
        if !meta.authority_bundle.is_empty() {
            let auth_json = meta.authority_bundle.to_json();
            if let Err(e) = update_entity_authority_ids(&pool, entity_id, &auth_json).await {
                tracing::warn!(error = %e, "failed to persist authority bundle");
            }
        }
    }

    // Subject is "confirmed alive" if Wikidata was successfully queried but returned no death date.
    // This prevents noisy Wikipedia extractions from creating spurious death events for living people.
    let subject_confirmed_alive = wd_meta
        .as_ref()
        .map(|m| m.death_year.is_none())
        .unwrap_or(false);

    let (db_birth_year, db_death_year, has_birth, has_death) =
        quality_lifespan_years(&pool, entity_id).await?;
    let mut ctx = GateContext {
        subject_birth_year: wd_meta
            .as_ref()
            .and_then(|m| m.birth_year)
            .or(db_birth_year),
        subject_death_year: wd_meta
            .as_ref()
            .and_then(|m| m.death_year)
            .or(db_death_year),
        has_active_birth: has_birth,
        has_active_death: has_death,
        subject_confirmed_alive,
        ..Default::default()
    };

    if let Some(meta) = wd_meta.as_ref() {
        if !meta.statements_text.is_empty() {
            let (doc, mut raw) = extract::statements_to_raw_items(subject, &meta.statements_text);
            extract::apply_wikidata_identity_places(
                &mut raw,
                meta.birth_place_label.as_deref(),
                meta.death_place_label.as_deref(),
            );
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
                            document_text: None,
                            section_heading: None,
                        },
                    )
                    .await?,
                    &mut facts_inserted,
                    &mut facts_reinforced,
                );
            }
        }
    }

    // Phase 1: Core subject pages (Core priority - processed immediately)
    if let Some(ref p) = progress {
        p.set_phase("core_extract").await;
    }

    for lang in collect::wiki_langs(wiki_lang) {
        match collect::fetch_wiki_extract(&lang, subject).await {
            Ok((title, text, links)) => {
                // Key by lang+title so EN and FR subject pages both ingest
                // (same display title must not skip the other language).
                let lang_title_key = format!("{lang}:{title}");
                if crawl_queue.is_seen(&lang_title_key) {
                    continue;
                }
                crawl_queue.mark_seen(&lang_title_key);
                crawl_queue.mark_seen(&title);
                if wiki_pages == 0 {
                    primary_title = title.clone();
                }
                wiki_pages += 1;
                if let Some(ref p) = progress {
                    p.set_phase_with_page("core_extract", &title).await;
                    p.increment_wiki_pages().await;
                }
                // Add page links to queue with appropriate priorities
                crawl_queue.extend(crawl_items_from_page_links(&links, &title, subject, follow_cap));
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
            let (doc, mut raw) = extract::statements_to_raw_items(subject, &meta.statements_text);
            extract::apply_wikidata_identity_places(
                &mut raw,
                meta.birth_place_label.as_deref(),
                meta.death_place_label.as_deref(),
            );
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
                            document_text: None,
                            section_heading: None,
                        },
                    )
                    .await?,
                    &mut facts_inserted,
                    &mut facts_reinforced,
                );
            }
        }
    }

    if let Some(ref p) = progress {
        p.set_phase("wdqs").await;
    }

    match fetch_events_for_person(&resolved_qid).await {
        Ok(events) => {
            wdqs_events = events.len() as u32;
            if let Some(ref p) = progress {
                p.set_wdqs_events(wdqs_events).await;
            }
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
            // WDQS events get High priority - direct subject participation
            crawl_queue.extend(crawl_items_from_wdqs(&events, subject, follow_cap));
        }
        Err(err) => tracing::warn!(error = %err, "wdqs participation harvest failed"),
    }

    // Add seed list items with appropriate priorities
    if let Some(path) = seed_list {
        match load_seed_titles(path) {
            Ok(seeds) => {
                crawl_queue.extend(crawl_items_from_seed_list(seeds, subject));
            }
            Err(err) => tracing::debug!(error = %err, "seed list not loaded"),
        }
    }

    // Sort queue by priority before processing follow pages
    crawl_queue.sort_by_priority();

    // Update progress with queue stats
    if let Some(ref p) = progress {
        p.set_priority_counts(crawl_queue.count_by_priority()).await;
    }

    // Phase 2+3: Process follow pages in priority order
    // High priority pages (birth/death places, WDQS events) first,
    // then Medium (battles, treaties), then Low (generic links)
    let extra = follow_cap.saturating_sub(wiki_pages);
    let mut followed = 0u32;
    let mut fetched = wiki_pages;
    let mut attempts = 0u32;
    let attempt_cap = extra.saturating_mul(4).max(extra);
    let mut current_priority: Option<CrawlPriority> = None;

    while let Some(item) = crawl_queue.pop() {
        if followed >= extra || attempts >= attempt_cap {
            break;
        }

        // Update phase when priority tier changes
        if current_priority != Some(item.priority) {
            current_priority = Some(item.priority);
            let phase = item.priority.phase_name();
            if let Some(ref p) = progress {
                p.set_phase(phase).await;
                p.set_priority_counts(crawl_queue.count_by_priority()).await;
            }
        }

        if !collect::should_pin_follow_title(&item.title) {
            continue;
        }

        if let Some(ref p) = progress {
            p.set_phase_with_page(
                current_priority.map_or("expand_links", |pr| pr.phase_name()),
                &item.title,
            )
            .await;
            p.set_sources_pending(crawl_queue.remaining() as u32).await;
        }

        attempts += 1;
        fetched += 1;
        match ingest_follow_page(
            &pool,
            entity_id,
            subject,
            wiki_lang,
            &item.title,
            &mut ctx,
            &aliases,
            military_subject,
        )
        .await
        {
            Ok((ins, re)) => {
                if ins + re > 0 {
                    followed += 1;
                    wiki_pages += 1;
                    if let Some(ref p) = progress {
                        p.increment_wiki_pages().await;
                    }
                }
                facts_inserted += ins;
                facts_reinforced += re;
            }
            Err(err) => tracing::debug!(title = %item.title, error = %err, "follow wikipedia page skipped"),
        }
    }

    if let Some(ref p) = progress {
        p.set_phase("grounding").await;
    }

    typing::backfill_person_geocodes(&pool, entity_id).await?;

    if let Some(ref p) = progress {
        p.set_phase("corpus_enrichment").await;
    }

    // Catalog discovery records document candidates. A bibliographic notice is not
    // event evidence and does not reinforce an event it does not narrate.
    let corpus_stats = {
        let resolved_subject = talaria_sources::ResolvedSubject {
            entity_id: Some(entity_id),
            qid: Some(resolved_qid.clone()),
            label: subject.to_string(),
            languages: vec![wiki_lang.to_string()],
            birth_year: ctx.subject_birth_year,
            death_year: ctx.subject_death_year,
            countries: vec![],
            occupations: wd_meta
                .as_ref()
                .map(|m| m.occupations.clone())
                .unwrap_or_default(),
            known_identifiers: vec![],
        };
        let corpus_config = corpus_evidence::CorpusEnrichmentConfig::default();
        match corpus_evidence::enrich_with_corpus_evidence(
            &pool,
            entity_id,
            &resolved_subject,
            &corpus_config,
        )
        .await
        {
            Ok(stats) => Some(stats),
            Err(e) => {
                tracing::warn!(error = %e, "corpus evidence enrichment failed");
                None
            }
        }
    };

    Ok(json!({
        "lane": "explorer",
        "pipeline": "person",
        "subject": subject,
        "qid": resolved_qid,
        "entity_id": entity_id,
        "wikipedia_title": primary_title,
        "wiki_pages": wiki_pages,
        "documents_fetched": fetched,
        "documents_used": wiki_pages,
        "follow_attempts": attempts,
        "wdqs_events": wdqs_events,
        "facts_inserted": facts_inserted,
        "facts_reinforced": facts_reinforced,
        "debates_inserted": debates_inserted,
        "llm_configured": llm::is_configured(),
        "llm_model": llm::model(),
        "dropped_chunks": dropped,
        "corpus_enrichment": corpus_stats.map(|s| json!({
            "sources_queried": s.sources_queried,
            "documents_discovered": s.documents_discovered,
            "documents_matched": s.documents_matched,
            "evidence_added": s.evidence_added,
            "candidate_links_added": s.candidate_links_added,
            "errors": s.errors,
        })),
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

pub(crate) async fn ingest_wiki_text(
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
                    document_text: Some(text),
                    section_heading: None,
                },
            )
            .await?,
            &mut inserted,
            &mut reinforced,
        );
    }
    // Rules used to return here and skip the LLM, so biographies stayed thin.
    // Four chunks is the budget for one page. A longer article stays partial.
    for chunk in extract::split_chunks(text, 3500).into_iter().take(MAX_LLM_CHUNKS_PER_PAGE) {
        let extracted = if llm::is_configured() {
            match extract::extract_prose_chunk(pool, raw_id, subject, title, &chunk).await {
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
                                document_text: Some(&chunk),
                                section_heading: None,
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
                    document_text: Some(&doc),
                    section_heading: None,
                },
            )
            .await?,
            &mut inserted,
            &mut reinforced,
        );
    }
    for item in grounding::ground_prose(
        subject,
        &doc,
        extract::extract_wiki_rules(subject, &resolved, &doc, ctx.subject_death_year),
    ) {
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
                    primary_object: Some(resolved.as_str()),
                    source_locator: &locator,
                    page_title: resolved.as_str(),
                    from_followed_page: true,
                    structured_source: false,
                    military_subject,
                    aliases,
                    document_text: Some(&doc),
                    section_heading: None,
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
                        document_text: None,
                        section_heading: None,
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

#[cfg(test)]
mod qualification {
    use super::*;
    use persist::PersistMeta;
    use talaria_quality::{suggest_chapters, Lane, RawExtractItem};

    const FR: &str = include_str!("fixtures/desbordes-fr.txt");
    const EN: &str = include_str!("fixtures/desbordes-en.txt");
    const WD: &str = include_str!("fixtures/desbordes-wd.txt");
    const WD_PART: &str = include_str!("fixtures/desbordes-wd-participation.txt");

    fn pipe_item(line: &str) -> Option<RawExtractItem> {
        let parts: Vec<&str> = line.split(" | ").map(str::trim).collect();
        if parts.len() < 4 || parts[1].is_empty() {
            return None;
        }
        let when = parts[3];
        let year = when.chars().take(4).collect::<String>().parse::<i32>().ok();
        let place = parts.get(4).map(|value| value.trim()).filter(|value| !value.is_empty());
        Some(RawExtractItem {
            lane: "fact".into(),
            event_type: parts[1].to_string(),
            role: parts[2].to_string(),
            year,
            place_surface: place.map(str::to_string),
            summary: line.to_string(),
            quoted_text: line.to_string(),
            confidence: 0.9,
        })
    }

    async fn replay_once(pool: &sqlx::PgPool, entity_id: Uuid, ctx: &mut GateContext) {
        for (uri, text) in [
            ("https://www.wikidata.org/wiki/Q466987", WD),
            ("https://www.wikidata.org/wiki/Q466987#participation", WD_PART),
        ] {
            let raw_id = upsert_raw_wikidata_document(pool, uri, "Marceline Desbordes-Valmore", text)
                .await
                .unwrap();
            let items: Vec<RawExtractItem> = text.lines().filter_map(pipe_item).collect();
            for item in grounding::ground_structured(
                "Marceline Desbordes-Valmore",
                text,
                items,
                Some("Q466987"),
            ) {
                if item.lane != Lane::Fact {
                    continue;
                }
                let _ = persist::persist_fact_item(
                    pool,
                    entity_id,
                    "Marceline Desbordes-Valmore",
                    &item,
                    ctx,
                    PersistMeta {
                        raw_document_id: raw_id,
                        coords: None,
                        primary_object: None,
                        source_locator: uri,
                        page_title: "Marceline Desbordes-Valmore",
                        from_followed_page: false,
                        structured_source: true,
                        military_subject: false,
                        aliases: &[],
                        document_text: None,
                        section_heading: None,
                    },
                )
                .await
                .unwrap();
            }
        }
        for (lang, text) in [("fr", FR), ("en", EN)] {
            ingest_wiki_text(
                pool,
                entity_id,
                "Marceline Desbordes-Valmore",
                lang,
                "Marceline Desbordes-Valmore",
                text,
                ctx,
                &[],
                false,
            )
            .await
            .unwrap();
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn the_original_desbordes_documents_replay_without_the_old_errors(pool: sqlx::PgPool) {
        std::env::remove_var("TALARIA_OFFLINE_PLACES");
        std::env::set_var("TALARIA_GEOCODE_BUDGET", "4");
        for key in ["OPENAI_API_KEY", "OPENROUTER_API_KEY", "LLM_API_KEY"] {
            unsafe { std::env::remove_var(key) };
        }
        assert!(!crate::llm::is_configured(), "the comparison stays on the rules, not a model");
        let entity_id = talaria_store::ensure_subject_entity(
            &pool,
            "Q466987",
            "Marceline Desbordes-Valmore",
        )
        .await
        .unwrap();
        let mut ctx = GateContext {
            subject_birth_year: Some(1786),
            subject_death_year: Some(1859),
            has_active_birth: false,
            has_active_death: false,
            ..Default::default()
        };
        replay_once(&pool, entity_id, &mut ctx).await;
        let first: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM canonical_events WHERE entity_id = $1 AND is_active",
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        replay_once(&pool, entity_id, &mut ctx).await;
        let second: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM canonical_events WHERE entity_id = $1 AND is_active",
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(first, second, "a second pass must not create new public facts");

        let bad: Vec<String> = sqlx::query_scalar(
            "SELECT left(ee.quoted_text, 240) FROM canonical_events ce \
             JOIN event_evidence ee ON ee.canonical_event_id = ce.id \
             WHERE ce.entity_id = $1 AND ce.is_active AND (\
               ee.quoted_text ILIKE '%Hyacinthe%' \
               OR ee.quoted_text ILIKE '%Gallica%' \
               OR ee.quoted_text ILIKE '%Houssin%' \
               OR ee.quoted_text ILIKE '%Impr.%' \
               OR ee.quoted_text ILIKE '%Imprimerie%' \
               OR coalesce(ce.place_label, '') ILIKE '%criture%'\
             )",
        )
        .bind(entity_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(bad.is_empty(), "{bad:?}");
        let mapped_country: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM canonical_events \
             WHERE entity_id = $1 AND is_active AND map_eligible AND place_label = 'France'",
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(mapped_country, 0);
        let borrowed_birth: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM canonical_events ce \
             JOIN event_evidence ee ON ee.canonical_event_id = ce.id \
             WHERE ce.entity_id = $1 AND ce.is_active \
               AND ee.quoted_text ILIKE '%back to France%' \
               AND ce.time_json->>'start' = '1786'",
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(borrowed_birth, 0);
        let elegies: i64 = sqlx::query_scalar(
            "SELECT count(DISTINCT ce.id)::bigint FROM canonical_events ce \
             JOIN event_evidence ee ON ee.canonical_event_id = ce.id \
             WHERE ce.entity_id = $1 AND ce.is_active \
               AND (ee.quoted_text ILIKE '%élégies et romances%' \
                 OR ee.quoted_text ILIKE '%elegies et romances%')",
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(elegies <= 1, "one publication must stay one event");
        let dated_voyage: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM canonical_events ce \
             JOIN event_evidence ee ON ee.canonical_event_id = ce.id \
             WHERE ce.entity_id = $1 AND ce.is_active \
               AND ee.quoted_text ILIKE '%Guadeloupe%' \
               AND coalesce(ce.time_json->>'start', '') LIKE '1802%'",
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(dated_voyage, 0, "the mother's death must not date the voyage");
        let two_theatres: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM canonical_events ce \
             JOIN event_evidence ee ON ee.canonical_event_id = ce.id \
             WHERE ce.entity_id = $1 AND ce.is_active AND ce.map_eligible \
               AND ee.quoted_text ILIKE '%déon%' AND ee.quoted_text ILIKE '%Bruxelles%'",
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(two_theatres, 0, "two theatres are not one departure point");
        let places: Vec<(String, bool, Option<f64>)> = sqlx::query_as(
            "SELECT coalesce(place_label, ''), map_eligible, ST_Y(geom::geometry) \
             FROM canonical_events WHERE entity_id = $1 AND is_active",
        )
        .bind(entity_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(
            places.iter().any(|(place, mapped, _)| {
                *mapped && matches!(place.as_str(), "Douai" | "Lyon" | "Paris")
            }),
            "a demonstrated city must be mapped: {places:?}"
        );

        let rows: Vec<(String, String, String, Option<String>, bool, String)> = sqlx::query_as(
            "SELECT ce.event_type, coalesce(ce.place_label, ''), ce.time_json::text, \
                    ce.time_json->>'start', ce.map_eligible, left(ee.quoted_text, 220) \
             FROM canonical_events ce \
             JOIN LATERAL (SELECT quoted_text FROM event_evidence \
                           WHERE canonical_event_id = ce.id ORDER BY created_at LIMIT 1) ee ON true \
             WHERE ce.entity_id = $1 AND ce.is_active \
             ORDER BY ce.start_time NULLS LAST, ce.event_type",
        )
        .bind(entity_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        let counts: Vec<(String, i64)> = sqlx::query_as(
            "SELECT event_type, count(*)::bigint FROM canonical_events \
             WHERE entity_id = $1 AND is_active GROUP BY event_type",
        )
        .bind(entity_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        let pairs: Vec<(&str, usize)> = counts
            .iter()
            .map(|(kind, count)| (kind.as_str(), *count as usize))
            .collect();
        let suggestions = suggest_chapters(&pairs);
        let published: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM editorial_groups WHERE entity_id = $1 AND status = 'published'",
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(published, 0);
        let statuses: Vec<(String, i64)> = sqlx::query_as(
            "SELECT status, count(*)::bigint FROM event_candidates \
             WHERE subject_entity_id = $1 GROUP BY status ORDER BY status",
        )
        .bind(entity_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        let mut audit = format!("public={first} suggestions={}\n", suggestions.len());
        for (status, count) in &statuses {
            audit.push_str(&format!("candidates {status} {count}\n"));
        }
        for suggestion in &suggestions {
            audit.push_str(&format!("chapter {} {}\n", suggestion.slug, suggestion.event_count));
        }
        for (kind, place, time_json, start, mapped, quote) in &rows {
            audit.push_str(&format!("{kind}\t{place}\t{start:?}\t{mapped}\t{time_json}\t{quote}\n"));
        }
        let watched: Vec<(String, String)> = sqlx::query_as(
            "SELECT status, coalesce(judgment_json->>'public_admission','') || ' ' || coalesce(judgment_json->>'attribution','') || ' ' || left(evidence_ptrs::text, 120) \
             FROM event_candidates WHERE subject_entity_id = $1 AND (\
               evidence_ptrs::text ILIKE '%louette%' OR judgment_json::text ILIKE '%louette%' \
               OR evidence_ptrs::text ILIKE '%Poésies de Mme%' OR judgment_json::text ILIKE '%Poésies de Mme%' \
               OR evidence_ptrs::text ILIKE '%23 juillet%' OR judgment_json::text ILIKE '%23 juillet%' \
               OR evidence_ptrs::text ILIKE '%Le Billet%' OR judgment_json::text ILIKE '%Le Billet%')",
        )
        .bind(entity_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        for (status, excerpt) in &watched {
            audit.push_str(&format!("watched {status} {excerpt}\n"));
        }
        std::fs::write("/tmp/v311e-desbordes-audit.txt", &audit).unwrap();
        assert!(first > 0, "the replay published nothing\n{audit}");
    }

    #[tokio::test]
    async fn write_the_qualification_database_when_asked() {
        let Ok(url) = std::env::var("TALARIA_QUALIFICATION_DB") else {
            return;
        };
        if !url.contains("v311e_ephemeral") {
            return;
        }
        std::env::remove_var("TALARIA_OFFLINE_PLACES");
        std::env::set_var("TALARIA_GEOCODE_BUDGET", "4");
        for key in ["OPENAI_API_KEY", "OPENROUTER_API_KEY", "LLM_API_KEY"] {
            unsafe { std::env::remove_var(key) };
        }
        let pool = sqlx::PgPool::connect(&url).await.unwrap();
        let entity_id = talaria_store::ensure_subject_entity(
            &pool,
            "Q466987",
            "Marceline Desbordes-Valmore",
        )
        .await
        .unwrap();
        let mut ctx = GateContext {
            subject_birth_year: Some(1786),
            subject_death_year: Some(1859),
            has_active_birth: false,
            has_active_death: false,
            ..Default::default()
        };
        replay_once(&pool, entity_id, &mut ctx).await;
        let public_count: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM canonical_events WHERE entity_id = $1 AND is_active",
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(public_count > 0);
    }
}
