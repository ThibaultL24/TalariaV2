// crates/talaria-api/src/lot_e.rs
//! Lot E: dense seed-driven quality ingest toward density targets.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use talaria_core::AppConfig;
use talaria_dump::content_hash;
use talaria_quality::{
    apply_gates, candidate_fingerprint, existing_candidate_action, occurrence_key_for_event,
    occurrence_stem_for_event, parse_typed_time, resolve_mentions, should_reinforce_existing_event,
    start_time_from_typed, time_to_json, BuildProjections, DerivedLabelProjections, EntityKind,
    EvidencePtr, ExistingCandidateAction, EXTRACTOR_EPISTEMIC_STATUS, GazetteerResolver,
    GateContext, ASSEMBLER_V1,
};
use talaria_sources::extractors::{
    claim_fingerprint, extractor_stack_for_classes, CandidateExtractor, ClaimKey, ExtractorInput,
};
use talaria_sources::connectors::net::send_retrying;
use talaria_sources::{
    filter_wiki_titles_for_classes, first_year_in_window, is_plausible_place_label,
    lifespan_year_window, load_seed_titles, merge_seed_titles_for, place_hint_from_title,
    rank_wikipedia_title_for_classes, resolve_place_offline, DensityProgress, DensityTargets,
    ResolvedSubject,
};
use talaria_store::{
    add_claim_support, apply_place_to_quality_event, connect, density_report_counts,
    find_active_quality_event_by_occurrence_key, get_event_candidate_by_fingerprint,
    insert_document_fragment, insert_document_snapshot, insert_quality_canonical_event,
    link_claim_to_event, mark_candidate_assembled, quality_lifespan_years,
    reinforce_quality_event, reject_if_singleton_exists, run_migrations,
    update_event_candidate_judgment, upsert_entity_with_kind, upsert_event_candidate,
    upsert_quality_claim, DocumentFragmentInsert, DocumentSnapshotInsert, EventCandidateInsert,
    QualityClaimInsert, QualityEventInsert,
};

use crate::cli_helpers::open_db_for_subject;

fn event_type_from_page_title(title: &str) -> &'static str {
    let lower = title.to_lowercase();
    if lower.starts_with("siege") || lower.starts_with("siège") {
        "siege"
    } else if lower.starts_with("treaty") || lower.starts_with("traité") {
        "treaty"
    } else {
        "battle"
    }
}
use uuid::Uuid;

use crate::place_conflict::{abstain_if_competing_place, competing_place_codes};

fn html_to_rough_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut in_tag = false;
    let mut prev_space = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                if !prev_space {
                    out.push(' ');
                    prev_space = true;
                }
            }
            _ if in_tag => {}
            c if c.is_whitespace() => {
                if !prev_space {
                    out.push(' ');
                    prev_space = true;
                }
            }
            c => {
                out.push(c);
                prev_space = false;
            }
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

async fn fetch_wikipedia_extract_rest(
    client: &reqwest::Client,
    lang: &str,
    title: &str,
) -> anyhow::Result<(String, String, Option<String>, Option<(f64, f64)>)> {
    let title_path = title.replace(' ', "_");
    let mut html_url = reqwest::Url::parse(&format!("https://{lang}.wikipedia.org"))?;
    {
        let mut segs = html_url
            .path_segments_mut()
            .map_err(|_| anyhow::anyhow!("bad wikipedia base url"))?;
        segs.extend(["api", "rest_v1", "page", "html", &title_path]);
    }
    let html_resp = send_retrying(client.get(html_url), 8, Duration::from_secs(4)).await.map_err(|e| anyhow::anyhow!("{e}"))?.error_for_status()?;
    let html = html_resp.text().await?;
    let extract = html_to_rough_text(&html);
    if extract.len() < 80 {
        anyhow::bail!("rest html too short for {title}");
    }

    let mut resolved = title.to_string();
    let mut coords = None;
    let mut summary_url = reqwest::Url::parse(&format!("https://{lang}.wikipedia.org"))?;
    {
        let mut segs = summary_url
            .path_segments_mut()
            .map_err(|_| anyhow::anyhow!("bad wikipedia base url"))?;
        segs.extend(["api", "rest_v1", "page", "summary", &title_path]);
    }
    if let Ok(sum) = client.get(summary_url).send().await {
        if sum.status().is_success() {
            if let Ok(v) = sum.json::<serde_json::Value>().await {
                if let Some(t) = v.get("title").and_then(|x| x.as_str()) {
                    resolved = t.to_string();
                }
                coords = v.get("coordinates").and_then(|c| {
                    let lat = c.get("lat")?.as_f64()?;
                    let lon = c.get("lon")?.as_f64()?;
                    Some((lat, lon))
                });
            }
        }
    }
    Ok((resolved, extract, None, coords))
}

async fn fetch_wikipedia_extract(
    lang: &str,
    title: &str,
) -> anyhow::Result<(String, String, Option<String>, Option<(f64, f64)>)> {
    let client = wiki_http_client()?;
    let api = format!("https://{lang}.wikipedia.org/w/api.php");
    let action = send_retrying(
        client.get(&api).query(&[
            ("action", "query"),
            ("prop", "extracts|info|coordinates"),
            ("explaintext", "1"),
            ("exlimit", "1"),
            ("titles", title),
            ("format", "json"),
            ("redirects", "1"),
            ("colimit", "1"),
        ]),
        8,
        Duration::from_secs(4),
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e}"));

    match action {
        Ok(resp) if resp.status().is_success() => {
            let response = resp.json::<serde_json::Value>().await?;
            let pages = response
                .pointer("/query/pages")
                .and_then(|v| v.as_object())
                .ok_or_else(|| anyhow::anyhow!("no pages"))?;
            let page = pages
                .values()
                .next()
                .ok_or_else(|| anyhow::anyhow!("empty pages"))?;
            if page.get("missing").is_some() {
                anyhow::bail!("missing page {title}");
            }
            let extract = page
                .get("extract")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let resolved = page
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or(title)
                .to_string();
            let revid = page
                .get("lastrevid")
                .and_then(|v| v.as_u64())
                .map(|n| n.to_string());
            let coords = page
                .get("coordinates")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|c| {
                    let lat = c.get("lat")?.as_f64()?;
                    let lon = c.get("lon")?.as_f64()?;
                    Some((lat, lon))
                });
            if !extract.is_empty() {
                return Ok((resolved, extract, revid, coords));
            }
        }
        Ok(resp) if resp.status().as_u16() == 429 || resp.status().as_u16() == 503 => {
            anyhow::bail!("wikipedia still rate-limited for {title}");
        }
        Ok(resp) => {
            tracing::warn!(
                %title,
                status = %resp.status(),
                "wikipedia action API failed — trying REST html fallback"
            );
        }
        Err(err) => {
            tracing::warn!(%title, error = %err, "wikipedia action API error — trying REST html fallback");
        }
    }

    fetch_wikipedia_extract_rest(&client, lang, title).await
}

#[derive(Debug, Default)]
struct LotEMetrics {
    titles_attempted: u32,
    titles_fetched: u32,
    titles_failed: u32,
    candidates: u32,
    accepted: u32,
    rejected: u32,
    events_created: u32,
    events_reinforced: u32,
    map_resolved: u32,
    loss: std::collections::BTreeMap<String, u32>,
}

impl LotEMetrics {
    fn bump(&mut self, k: &str) {
        *self.loss.entry(k.to_string()).or_insert(0) += 1;
    }
}

pub async fn run_lot_e_density_ingest(
    config: &AppConfig,
    subject: &str,
    qid: Option<&str>,
    seed_list: &Path,
    targets: DensityTargets,
    lang: &str,
    max_titles: Option<u32>,
) -> anyhow::Result<String> {
    let (pool, subject_id) = open_db_for_subject(config, subject, "person").await?;
    if let Some(qid) = qid {
        talaria_store::update_entity_qid(&pool, subject_id, qid).await?;
    }
    let (by, dy, _, _) = quality_lifespan_years(&pool, subject_id).await?;
    let wd_meta = match qid {
        Some(q) => match fetch_wikidata_subject_meta(q, lang).await {
            Ok(meta) => meta,
            Err(e) => {
                tracing::warn!(error = %e, %q, "wikidata subject meta failed — continuing without it");
                WikidataSubjectMeta::default()
            }
        },
        None => WikidataSubjectMeta::default(),
    };
    // Wikidata P569/P570 is the lifespan source of truth when present.
    // Never keep a noisy quality birth/death that contradicts it (gates would reject the life).
    if wd_meta.birth_year.is_some() || wd_meta.death_year.is_some() {
        let n = deactivate_mismatched_lifespan(
            &pool,
            subject_id,
            wd_meta.birth_year,
            wd_meta.death_year,
        )
        .await
        .unwrap_or(0);
        if n > 0 {
            tracing::info!(n, "deactivated quality birth/death that disagree with Wikidata");
        }
    }
    let mut subject_res = ResolvedSubject {
        entity_id: Some(subject_id),
        qid: qid.map(str::to_string),
        label: subject.into(),
        languages: vec![lang.into(), "en".into(), "fr".into()],
        birth_year: wd_meta.birth_year.or(by),
        death_year: wd_meta.death_year.or(dy),
        countries: vec![],
        occupations: wd_meta.occupations.clone(),
        known_identifiers: qid
            .map(|q| vec![("wikidata".into(), q.to_string())])
            .unwrap_or_default(),
    };

    let person_classes = subject_res.person_classes();
    let person_class = subject_res.person_class();
    let military_signal = subject_res.has_military_signal();
    tracing::info!(
        class = person_class.as_str(),
        facets = ?person_classes.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
        occupations = ?subject_res.occupations,
        military_signal,
        "resolved person ingest classes"
    );

    let mut titles = load_seed_titles(seed_list).unwrap_or_default();
    if titles.is_empty() {
        titles.push(subject.to_string());
    }
    if let Some(wiki_title) = &wd_meta.wiki_title {
        if !titles.iter().any(|t| t.eq_ignore_ascii_case(wiki_title)) {
            titles.insert(0, wiki_title.clone());
        }
    }
    let expand_cap = max_titles
        .filter(|n| *n > 0)
        .unwrap_or(targets.max_documents.min(400)) as usize;
    let start_title = wd_meta
        .wiki_title
        .clone()
        .unwrap_or_else(|| subject.to_string());
    let wiki_links = fetch_wikipedia_article_links(lang, &start_title, expand_cap)
        .await
        .unwrap_or_default();
    let cap = expand_cap.max(titles.len());
    titles = merge_seed_titles_for(
        subject,
        titles,
        wiki_links
            .into_iter()
            .chain(wd_meta.related_titles.clone()),
        cap,
        military_signal,
    );
    titles = filter_wiki_titles_for_classes(
        subject,
        titles,
        &person_classes,
        subject_res.death_year,
        military_signal,
    );
    tracing::info!(
        seeds = titles.len(),
        class = person_class.as_str(),
        birth = ?subject_res.birth_year,
        death = ?subject_res.death_year,
        occupations = ?subject_res.occupations,
        military_signal,
        "expanded Wikipedia/Wikidata seeds ranked by person facets"
    );
    if let Some(max) = max_titles {
        titles.truncate(max as usize);
    }

    // Soft-clean implausible place noise from earlier extractor versions before counting.
    let cleaned = deactivate_implausible_place_events(config, subject)
        .await
        .unwrap_or(0);
    if cleaned > 0 {
        tracing::info!(
            cleaned,
            "deactivated quality events with implausible place labels"
        );
    }

    let extractors = extractor_stack_for_classes(&person_classes, military_signal);
    let extractor_refs: Vec<&dyn CandidateExtractor> =
        extractors.iter().map(|e| e.as_ref()).collect();
    let resolver = GazetteerResolver;
    let projections = DerivedLabelProjections;
    let mut metrics = LotEMetrics::default();

    if !wd_meta.statements_text.is_empty() {
        if let Err(e) = ingest_memory_document(
            &pool,
            config,
            &subject_res,
            subject_id,
            subject,
            "wikidata",
            &format!("https://www.wikidata.org/wiki/{}", qid.unwrap_or("")),
            "structured_statement",
            &wd_meta.statements_text,
            None,
            &extractor_refs,
            &resolver,
            &projections,
            &mut metrics,
        )
        .await
        {
            tracing::warn!(error = %e, "wikidata structured statements ingest failed");
        }
        let (by2, _, _, _) = quality_lifespan_years(&pool, subject_id).await?;
        if subject_res.birth_year.is_none() {
            subject_res.birth_year = by2;
        }
    }

    if let Some(qid) = subject_res.qid.clone() {
        match crate::ingest::ingest_wdqs_events(&pool, config, &subject_res, subject_id).await {
            Ok(wdqs) => {
                tracing::info!(
                    created = wdqs.events_created,
                    accepted = wdqs.accepted,
                    %qid,
                    "WDQS participation and biography events ingested"
                );
                metrics.events_created += wdqs.events_created as u32;
                metrics.accepted += wdqs.accepted as u32;
                metrics.rejected += wdqs.rejected as u32;
            }
            Err(e) => tracing::warn!(error = %e, %qid, "WDQS ingest failed"),
        }
    }

    let original_seeds: Vec<String> = titles.clone();
    let mut title_queue: std::collections::VecDeque<String> = titles.into_iter().collect();
    let mut seen_titles: std::collections::HashSet<String> = title_queue.iter().cloned().collect();
    let langs = vec![lang.to_string()]; // one language per run; re-run with --wiki-lang fr to densify further

    for current_lang in &langs {
        // Re-seed queue per language so FR also explores the same seeds + discoveries.
        if current_lang != lang {
            title_queue.clear();
            for t in &original_seeds {
                title_queue.push_back(t.clone());
            }
            for t in &seen_titles {
                if !original_seeds.iter().any(|o| o == t) {
                    title_queue.push_back(t.clone());
                }
            }
        }
        let density = density_report_counts(&pool, Some(subject_id)).await?;
        if density.map_eligible >= targets.target_map_events as i64
            && density.timeline_eligible >= targets.target_timeline_events as i64
        {
            break;
        }
        // Snapshot queue length at language start; discoveries append for later processing.
        let mut processed_in_lang = 0u32;
        while let Some(title) = title_queue.pop_front() {
            processed_in_lang += 1;
            if processed_in_lang > targets.max_documents_per_source {
                metrics.bump("budget_per_source");
                break;
            }
            // Check density progress
            let density = density_report_counts(&pool, Some(subject_id)).await?;
            let progress = DensityProgress {
                timeline_events: density.timeline_eligible as u32,
                map_events: density.map_eligible as u32,
                documents_processed: metrics.titles_fetched,
                target_reached: false,
                status: String::new(),
            }
            .evaluate(&targets);
            if progress.target_reached {
                tracing::info!(?progress, "density target reached — stopping exploration");
                break;
            }
            if metrics.titles_fetched as u32 >= targets.max_documents {
                metrics.bump("budget_documents");
                break;
            }

            metrics.titles_attempted += 1;
            let (resolved_title, text, revid, page_coords) =
                match fetch_wikipedia_extract(current_lang, &title).await {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!(%title, lang=%current_lang, error=%e, "fetch failed");
                        metrics.titles_failed += 1;
                        metrics.bump("fetch_failed");
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        continue;
                    }
                };
            metrics.titles_fetched += 1;
            // Be polite to Wikimedia — search-triggered density must not stampede.
            tokio::time::sleep(Duration::from_millis(2000)).await;

            // Battle/treaty regex expansion only when this person has a military signal.
            if military_signal {
                for linked in discover_linked_titles(&text) {
                    if rank_wikipedia_title_for_classes(
                        &linked,
                        &person_classes,
                        subject_res.death_year,
                        military_signal,
                    ) < 0.55
                    {
                        continue;
                    }
                    enqueue_discovered_title(
                        &mut title_queue,
                        &mut seen_titles,
                        linked,
                        targets.max_linked_entities as usize,
                    );
                }
            }
            // Prose-fragment title mining is disabled: it floods the queue with non-pages.
            // Growth comes from Wikipedia `prop=links` at ingest start instead.
            for linked in ([] as [String; 0]) {
                enqueue_discovered_title(
                    &mut title_queue,
                    &mut seen_titles,
                    linked,
                    targets.max_linked_entities as usize,
                );
            }

            let hash = content_hash(&text);
            let snapshot_id = insert_document_snapshot(
            &pool,
            &DocumentSnapshotInsert {
                source_type: "wikipedia".into(),
                source_uri: format!(
                    "https://{current_lang}.wikipedia.org/wiki/{}",
                    resolved_title.replace(' ', "_")
                ),
                source_identifier: Some(format!("{current_lang}:{resolved_title}")),
                language: current_lang.clone(),
                title: Some(resolved_title.clone()),
                content_hash: format!(
                    "{}:{}:{}",
                    current_lang,
                    hash,
                    revid.clone().unwrap_or_default()
                ),
                revision_id: revid,
                wiki_page_id: None,
                raw_document_id: None,
                text: text.clone(),
                metadata: serde_json::json!({
                    "seed": true,
                    "lot": "E",
                    "page_coords": page_coords.map(|(la, lo)| serde_json::json!({"lat": la, "lon": lo})),
                }),
            },
        )
        .await?;

            let frag_id = insert_document_fragment(
                &pool,
                &DocumentFragmentInsert {
                    snapshot_id,
                    fragment_kind: "sentence".into(),
                    parent_fragment_id: None,
                    sentence_id: None,
                    text: text.clone(),
                    start_offset: 0,
                    end_offset: text.len() as i32,
                    clause_index: None,
                    ordinal: 0,
                },
            )
            .await?;

            // Keep Wikidata lifespan sticky — quality refresh may still be the noisy singleton.
            let (by2, _, _, _) = quality_lifespan_years(&pool, subject_id).await?;
            if subject_res.birth_year.is_none() {
                subject_res.birth_year = by2;
            }

            let input = ExtractorInput {
                text: text.clone(),
                page_title: Some(resolved_title.clone()),
                subject_label: Some(subject.to_string()),
                document_type: "article".into(),
                subject_death_year: subject_res.death_year,
            };

            let mut raws = Vec::new();
            for ex in &extractor_refs {
                raws.extend(ex.extract(&input));
            }

            // For military subjects only: ensure battle/siege pages produce at least a
            // page-level candidate when the military_campaign extractor fired nothing.
            if military_signal
                && !raws.iter().any(|r| {
                    r.extractor_id == "military_campaign"
                        && r.object_surface.as_deref() == Some(resolved_title.as_str())
                })
            {
                if let Some(place) = place_hint_from_title(&resolved_title) {
                    if let Some(year) =
                        first_year_in(&text, subject_res.birth_year, subject_res.death_year)
                    {
                        let event_type = event_type_from_page_title(&resolved_title);
                        let predicate = match event_type {
                            "treaty" | "diplomatic" => "signed",
                            "siege" => "besieged",
                            _ => "fought_at",
                        };
                        raws.push(talaria_sources::extractors::RawCandidate {
                            event_type: event_type.into(),
                            predicate: predicate.into(),
                            subject_surface: subject.into(),
                            time_surface: Some(year),
                            place_surface: Some(place),
                            object_surface: Some(resolved_title.clone()),
                            participant_surfaces: vec![],
                            clause_text: resolved_title.clone(),
                            clause_index: 0,
                            start_offset: 0,
                            end_offset: 20,
                            cross_clause_join: false,
                            extractor_id: "page_fallback".into(),
                            is_posthumous: false,
                            lat: None,
                            lon: None,
                        });
                    }
                }
            }

            for raw in raws {
                process_one(
                    &pool,
                    config,
                    &subject_res,
                    subject_id,
                    snapshot_id,
                    frag_id,
                    &raw,
                    &resolver,
                    &projections,
                    page_coords,
                    &mut metrics,
                )
                .await?;
            }
        } // end queue while
    } // end langs

    // Resolve places for timeline-only events
    let unresolved: Vec<(Uuid, Option<String>)> = sqlx::query_as(
        r#"
        SELECT id, place_label FROM canonical_events
        WHERE pipeline = 'quality' AND is_active AND timeline_eligible AND NOT map_eligible
          AND entity_id = $1 AND place_label IS NOT NULL
        "#,
    )
    .bind(subject_id)
    .fetch_all(&pool)
    .await?;

    for (eid, label) in unresolved {
        if let Some(label) = label {
            if let Some(res) = resolve_place_offline(&label) {
                apply_place_to_quality_event(
                    &pool,
                    eid,
                    &label,
                    None,
                    res.lat,
                    res.lon,
                    &res.precision,
                    res.uncertainty_radius_m,
                )
                .await?;
                metrics.map_resolved += 1;
            }
        }
    }

    let density = density_report_counts(&pool, Some(subject_id)).await?;
    let progress = DensityProgress {
        timeline_events: density.timeline_eligible as u32,
        map_events: density.map_eligible as u32,
        documents_processed: metrics.titles_fetched,
        target_reached: false,
        status: String::new(),
    }
    .evaluate(&targets);

    let legacy_map: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint FROM canonical_events ce
        JOIN entities e ON e.id = ce.entity_id
        WHERE ce.pipeline = 'legacy' AND ce.map_eligible
          AND (e.wikipedia_title ILIKE $1 OR ce.title ILIKE $1)
        "#,
    )
    .bind(format!("%{subject}%"))
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    let report = serde_json::json!({
        "subject": {"label": subject, "qid": qid, "entity_id": subject_id},
        "targets": targets,
        "progress": progress,
        "run_metrics": {
            "titles_attempted": metrics.titles_attempted,
            "titles_fetched": metrics.titles_fetched,
            "titles_failed": metrics.titles_failed,
            "candidates": metrics.candidates,
            "accepted": metrics.accepted,
            "rejected": metrics.rejected,
            "events_created": metrics.events_created,
            "events_reinforced": metrics.events_reinforced,
            "places_resolved_deferred": metrics.map_resolved,
            "loss_reasons": metrics.loss,
        },
        "global": {
            "accepted_events": density.accepted_events,
            "timeline_eligible": density.timeline_eligible,
            "map_eligible": density.map_eligible,
            "events_without_place": density.events_without_place,
            "claims": density.claims,
            "candidates": density.candidates,
            "rejected": density.rejected,
            "multi_source": density.multi_source_events,
            "documents_snapshotted": density.documents_snapshotted,
        },
        "comparison": {
            "legacy_map_eligible": legacy_map,
            "quality_pr5_map": 18,
            "quality_lot_e_map": density.map_eligible,
            "quality_lot_e_timeline": density.timeline_eligible,
            "target_reached": progress.target_reached,
        },
        "connectors": {
            "wikipedia": "extraction_ready",
            "wikidata": "fetch_ready",
            "wikisource": "extraction_ready",
            "commons": "extraction_ready",
            "fixture": "production_ready",
            "bnf": "stub",
            "gallica": "extraction_ready",
            "europeana": "needs_EUROPEANA_API_KEY",
            "open_library": "extraction_ready",
            "internet_archive": "extraction_ready",
        }
    });
    let s = serde_json::to_string_pretty(&report)?;
    println!("{s}");
    Ok(s)
}

fn first_year_in(text: &str, birth: Option<i32>, death: Option<i32>) -> Option<String> {
    let (lo, hi) = lifespan_year_window(birth, death);
    first_year_in_window(text, lo, hi)
}

fn enqueue_discovered_title(
    queue: &mut std::collections::VecDeque<String>,
    seen: &mut std::collections::HashSet<String>,
    title: String,
    cap: usize,
) {
    if seen.len() >= cap {
        return;
    }
    if talaria_sources::is_noise_wiki_title(&title) {
        return;
    }
    if seen.insert(title.clone()) {
        queue.push_back(title);
    }
}

fn discover_linked_titles(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = text.to_lowercase();
    for (prefix, head) in [
        ("battle of ", "Battle of "),
        ("siege of ", "Siege of "),
        ("treaty of ", "Treaty of "),
        ("bataille de ", "Bataille de "),
        ("siège de ", "Siège de "),
        ("traite de ", "Traité de "),
        ("traité de ", "Traité de "),
    ] {
        let mut start = 0;
        while let Some(pos) = lower[start..].find(prefix) {
            let abs = start + pos;
            let after = &text[abs + prefix.len()..];
            let name = after
                .split(|c: char| {
                    c == '.' || c == ',' || c == ';' || c == '\n' || c == ')' || c == '('
                })
                .next()
                .unwrap_or("")
                .trim();
            if name.len() >= 2 && name.len() <= 60 && is_plausible_place_label(name) {
                out.push(format!("{head}{name}"));
            }
            start = abs + prefix.len();
            if out.len() > 80 {
                return out;
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
async fn process_one(
    pool: &sqlx::PgPool,
    config: &AppConfig,
    subject: &ResolvedSubject,
    subject_id: Uuid,
    snapshot_id: Uuid,
    frag_id: Uuid,
    raw: &talaria_sources::extractors::RawCandidate,
    resolver: &GazetteerResolver,
    projections: &DerivedLabelProjections,
    page_coords: Option<(f64, f64)>,
    metrics: &mut LotEMetrics,
) -> anyhow::Result<()> {
    let time = parse_typed_time(raw.time_surface.as_deref());
    let mut place_label = raw.place_surface.clone();
    if place_label.is_none() {
        if let Some(obj) = &raw.object_surface {
            place_label = place_hint_from_title(obj);
        }
    }
    if let Some(ref pl) = place_label {
        if !is_plausible_place_label(pl) {
            place_label = None;
        }
    }

    let mut shell = talaria_quality::EventCandidate {
        id: Uuid::nil(),
        snapshot_id,
        fragment_id: frag_id,
        clause_index: raw.clause_index,
        subject_surface: subject.label.clone(),
        subject_entity_id: Some(subject_id),
        event_type: raw.event_type.clone(),
        predicate: raw.predicate.clone(),
        time: time.clone(),
        place_mentions: vec![],
        object_mentions: vec![],
        participant_mentions: vec![],
        place_entity_id: None,
        place_label: place_label.clone(),
        evidence_ptrs: vec![EvidencePtr {
            fragment_id: frag_id,
            clause_index: raw.clause_index,
            start_offset: raw.start_offset,
            end_offset: raw.end_offset,
            quoted_text: raw.clause_text.clone(),
        }],
        extractor_version: raw.extractor_id.clone(),
        fingerprint: String::new(),
        status: talaria_quality::CandidateStatus::Pending,
        rejection_codes: vec![],
    };

    let resolved = resolve_mentions(
        &shell,
        resolver,
        place_label.as_deref(),
        raw.object_surface.as_deref(),
        &[],
    );
    // Prefer resolved place when valid; keep title-derived place otherwise.
    if resolved.place_kind == Some(EntityKind::Place) {
        shell.place_label = resolved.place_label.clone();
        shell.place_mentions = resolved.place_mentions;
    } else if resolved.invalid_place_attempt {
        // keep place_label from title hint if any; participants from resolve
        shell.participant_mentions = resolved.participant_mentions;
        shell.place_label = None;
    }

    // Resolve coordinates early for map eligibility
    let mut lat = None;
    let mut lon = None;
    let mut location_precision = None;
    let mut uncertainty = None;
    if raw.lat.is_some() && raw.lon.is_some() {
        lat = raw.lat;
        lon = raw.lon;
        location_precision = Some("wikidata_p625".into());
        if let Some(pl) = shell
            .place_label
            .clone()
            .or_else(|| raw.place_surface.clone())
            .filter(|s| !s.is_empty())
        {
            shell.place_label = Some(pl.clone());
            shell.place_entity_id =
                Some(upsert_entity_with_kind(pool, &config.wiki_lang, &pl, "place").await?);
        }
    } else if let Some(ref pl) = shell.place_label {
        if let Some(pres) = resolve_place_offline(pl) {
            lat = Some(pres.lat);
            lon = Some(pres.lon);
            location_precision = Some(pres.precision.clone());
            uncertainty = pres.uncertainty_radius_m;
            shell.place_entity_id =
                Some(upsert_entity_with_kind(pool, &config.wiki_lang, pl, "place").await?);
        } else if let Some((pla, plo)) = page_coords {
            // Wikipedia page coordinates — only for page-level / title-tied occurrences
            if raw.extractor_id == "military_campaign"
                || raw.extractor_id == "page_fallback"
                || raw.object_surface.as_deref() == Some(shell.place_label.as_deref().unwrap_or(""))
                || place_hint_from_title(raw.object_surface.as_deref().unwrap_or("")).is_some()
            {
                lat = Some(pla);
                lon = Some(plo);
                location_precision = Some("wikipedia_page_coordinates".into());
                uncertainty = Some(5000.0);
                shell.place_entity_id =
                    Some(upsert_entity_with_kind(pool, &config.wiki_lang, pl, "place").await?);
            }
        }
    } else if let Some((pla, plo)) = page_coords {
        // Page has coords but no place label — use title hint
        if let Some(hint) = place_hint_from_title(raw.object_surface.as_deref().unwrap_or(""))
            .or_else(|| place_hint_from_title(&raw.clause_text))
        {
            if is_plausible_place_label(&hint) {
                shell.place_label = Some(hint.clone());
                lat = Some(pla);
                lon = Some(plo);
                location_precision = Some("wikipedia_page_coordinates".into());
                uncertainty = Some(5000.0);
                shell.place_entity_id =
                    Some(upsert_entity_with_kind(pool, &config.wiki_lang, &hint, "place").await?);
            }
        }
    }

    let primary_object = raw.object_surface.clone();
    let occ = occurrence_key_for_event(
        &subject.label,
        &shell.event_type,
        &shell.predicate,
        &shell.time,
        shell.place_label.as_deref(),
        primary_object.as_deref(),
    );
    let stem = occurrence_stem_for_event(
        &subject.label,
        &shell.event_type,
        &shell.predicate,
        &shell.time,
        primary_object.as_deref(),
    );
    shell.fingerprint = candidate_fingerprint(
        &raw.extractor_id,
        &shell.subject_surface,
        &shell.event_type,
        &shell.predicate,
        &shell.time,
        shell.place_label.as_deref(),
        &snapshot_id.to_string(),
        shell.clause_index,
        raw.start_offset,
        raw.end_offset,
        &shell.participant_mentions,
    );

    let (cand_id, inserted) = upsert_event_candidate(
        pool,
        &EventCandidateInsert {
            snapshot_id,
            fragment_id: frag_id,
            clause_index: shell.clause_index,
            subject_surface: shell.subject_surface.clone(),
            subject_entity_id: shell.subject_entity_id,
            event_type: shell.event_type.clone(),
            predicate: shell.predicate.clone(),
            time_json: time_to_json(&shell.time),
            place_mentions: serde_json::to_value(&shell.place_mentions)?,
            object_mentions: serde_json::json!([]),
            participant_mentions: serde_json::to_value(&shell.participant_mentions)?,
            place_entity_id: shell.place_entity_id,
            place_label: shell.place_label.clone(),
            evidence_ptrs: serde_json::to_value(&shell.evidence_ptrs)?,
            extractor_version: shell.extractor_version.clone(),
            fingerprint: shell.fingerprint.clone(),
            occurrence_key: Some(occ.clone()),
            primary_object: primary_object.clone(),
            action_role: Some(shell.predicate.clone()),
            status: "pending".into(),
            rejection_codes: vec![],
            judgment_json: serde_json::json!({
                "occurrence_key": occ,
                "primary_object": primary_object
            }),
        },
    )
    .await?;
    let is_new_candidate = inserted;
    let mut skip_gates = false;
    if inserted {
        metrics.candidates += 1;
    } else {
        let existing = get_event_candidate_by_fingerprint(pool, &shell.fingerprint)
            .await?
            .ok_or_else(|| anyhow::anyhow!("fingerprint conflict without existing row"))?;
        match existing_candidate_action(&existing.status) {
            ExistingCandidateAction::SkipTerminal => return Ok(()),
            ExistingCandidateAction::ResumeAssembleOnly => {
                skip_gates = true;
            }
            ExistingCandidateAction::ResumeFromGates => {}
        }
    }

    if !skip_gates {
        let (birth_year, death_year, has_birth, has_death) =
            quality_lifespan_years(pool, subject_id).await?;
        let ctx = GateContext {
            subject_birth_year: subject.birth_year.or(birth_year),
            subject_death_year: subject.death_year.or(death_year),
            has_active_birth: has_birth,
            has_active_death: has_death,
            fingerprint_exists: false,
            cross_clause_join_detected: raw.cross_clause_join,
            place_entity_kind: if shell.place_entity_id.is_some() {
                Some(EntityKind::Place)
            } else {
                None
            },
        };
        let decision = apply_gates(&shell, &ctx);
        let status = decision.status().as_str();
        let codes = decision.codes();
        for c in &codes {
            metrics.bump(c);
        }
        update_event_candidate_judgment(
            pool,
            cand_id,
            status,
            &codes,
            &serde_json::json!({"codes": codes}),
            shell.subject_entity_id,
            shell.place_entity_id,
            shell.place_label.as_deref(),
            &serde_json::to_value(&shell.place_mentions)?,
            &serde_json::json!([]),
            &serde_json::to_value(&shell.participant_mentions)?,
        )
        .await?;

        match status {
            "accepted" => metrics.accepted += 1,
            "rejected" => {
                metrics.rejected += 1;
                return Ok(());
            }
            "needs_review" => return Ok(()),
            _ => {}
        }
    }

    let claim_fp = claim_fingerprint(&ClaimKey {
        subject: subject.label.clone(),
        predicate: shell.predicate.clone(),
        object_or_value: raw.object_surface.clone().unwrap_or_default(),
        time_key: shell.time.canonical_key(),
        place_key: shell.place_label.clone().unwrap_or_default(),
    });
    let (claim_id, _) = upsert_quality_claim(
        pool,
        &QualityClaimInsert {
            subject_entity_id: subject_id,
            fingerprint: claim_fp,
            predicate: shell.predicate.clone(),
            event_type: shell.event_type.clone(),
            object_json: serde_json::json!({"primary_object": raw.object_surface}),
            time_json: time_to_json(&shell.time),
            place_entity_id: shell.place_entity_id,
            place_label: shell.place_label.clone(),
            occurrence_stem: Some(stem.clone()),
        },
    )
    .await?;
    add_claim_support(
        pool,
        claim_id,
        Some(cand_id),
        Some(snapshot_id),
        "wikipedia",
        &serde_json::to_value(&shell.evidence_ptrs)?,
    )
    .await?;

    if let Some(places) =
        abstain_if_competing_place(pool, subject_id, &stem, shell.place_label.as_deref()).await?
    {
        let codes = competing_place_codes();
        update_event_candidate_judgment(
            pool,
            cand_id,
            "needs_review",
            &codes,
            &serde_json::json!({"abstain": true, "places": places}),
            shell.subject_entity_id,
            shell.place_entity_id,
            shell.place_label.as_deref(),
            &serde_json::to_value(&shell.place_mentions)?,
            &serde_json::json!([]),
            &serde_json::to_value(&shell.participant_mentions)?,
        )
        .await?;
        metrics.accepted = metrics.accepted.saturating_sub(1);
        metrics.bump("competing_place");
        return Ok(());
    }

    if let Some(existing) =
        find_active_quality_event_by_occurrence_key(pool, subject_id, &occ).await?
    {
        if should_reinforce_existing_event(is_new_candidate) {
            reinforce_quality_event(pool, existing).await?;
            metrics.events_reinforced += 1;
        }
        mark_candidate_assembled(pool, cand_id, existing).await?;
        link_claim_to_event(pool, claim_id, existing).await?;
        return Ok(());
    }

    if reject_if_singleton_exists(
        pool,
        cand_id,
        subject_id,
        &shell.event_type,
        shell.place_entity_id,
        shell.place_label.as_deref(),
        &serde_json::json!([]),
        &serde_json::json!([]),
        &serde_json::json!([]),
    )
    .await?
    {
        metrics.rejected += 1;
        metrics.accepted = metrics.accepted.saturating_sub(1);
        metrics.bump("singleton_cardinality_violation");
        return Ok(());
    }

    let map_eligible = lat.is_some() && lon.is_some();
    let proj = projections.from_candidate(&shell, &subject.label);
    let title_derived = projections.display_label(&proj);

    let event_id = insert_quality_canonical_event(
        pool,
        &QualityEventInsert {
            entity_id: subject_id,
            event_type: shell.event_type.clone(),
            epistemic_status: EXTRACTOR_EPISTEMIC_STATUS.into(),
            title: title_derived,
            summary: Some(raw.clause_text.clone()),
            start_time: start_time_from_typed(&shell.time),
            time_json: time_to_json(&shell.time),
            place_label: shell.place_label.clone(),
            place_entity_id: shell.place_entity_id,
            lat,
            lon,
            confidence: 0.75,
            map_eligible,
            historically_valid: true,
            timeline_eligible: true,
            fingerprint: occ.clone(),
            occurrence_key: Some(occ),
            occurrence_stem: Some(stem),
            primary_object,
            predicate: shell.predicate.clone(),
            assembler_version: ASSEMBLER_V1.into(),
            event_candidate_id: cand_id,
            supersedes: None,
            source_count: 1,
            evidence_count: 1,
        },
    )
    .await?;

    if map_eligible {
        if let (Some(la), Some(lo), Some(prec)) = (lat, lon, location_precision) {
            let _ = apply_place_to_quality_event(
                pool,
                event_id,
                shell.place_label.as_deref().unwrap_or(""),
                shell.place_entity_id,
                la,
                lo,
                &prec,
                uncertainty,
            )
            .await;
        }
    }

    mark_candidate_assembled(pool, cand_id, event_id).await?;
    link_claim_to_event(pool, claim_id, event_id).await?;
    metrics.events_created += 1;
    Ok(())
}

#[derive(Debug, Default, Clone)]
pub(crate) struct WikidataSubjectMeta {
    pub(crate) birth_year: Option<i32>,
    pub(crate) death_year: Option<i32>,
    pub(crate) occupations: Vec<String>,
    pub(crate) wiki_title: Option<String>,
    pub(crate) related_titles: Vec<String>,
    pub(crate) statements_text: String,
}

fn wiki_http_client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent("TalariaEngine/0.1 (lot-e density; research; contact: talaria-dev)")
        .timeout(Duration::from_secs(45))
        .build()?)
}


async fn deactivate_mismatched_lifespan(
    pool: &sqlx::PgPool,
    subject_id: Uuid,
    birth_year: Option<i32>,
    death_year: Option<i32>,
) -> anyhow::Result<u64> {
    let mut n = 0u64;
    if let Some(y) = birth_year {
        let r = sqlx::query(
            r#"
            UPDATE canonical_events SET is_active = false
            WHERE entity_id = $1 AND pipeline = 'quality' AND is_active
              AND event_type = 'birth'
              AND EXTRACT(YEAR FROM start_time) IS DISTINCT FROM $2
            "#,
        )
        .bind(subject_id)
        .bind(y)
        .execute(pool)
        .await?;
        n += r.rows_affected();
    }
    if let Some(y) = death_year {
        let r = sqlx::query(
            r#"
            UPDATE canonical_events SET is_active = false
            WHERE entity_id = $1 AND pipeline = 'quality' AND is_active
              AND event_type = 'death'
              AND EXTRACT(YEAR FROM start_time) IS DISTINCT FROM $2
            "#,
        )
        .bind(subject_id)
        .bind(y)
        .execute(pool)
        .await?;
        n += r.rows_affected();
    }
    Ok(n)
}

fn parse_wd_year(time: &str) -> Option<i32> {
    let t = time.trim();
    let negative = t.starts_with('-');
    let rest = t.trim_start_matches(['+', '-']);
    let year: i32 = rest.split('-').next()?.parse().ok()?;
    Some(if negative { -year } else { year })
}

#[cfg(test)]
mod parse_wd_year_tests {
    use super::parse_wd_year;

    #[test]
    fn ce_and_bce_wikidata_timestamps() {
        assert_eq!(parse_wd_year("+1769-08-15T00:00:00Z"), Some(1769));
        assert_eq!(parse_wd_year("-0069-01-01T00:00:00Z"), Some(-69));
        assert_eq!(parse_wd_year("+0001-01-01T00:00:00Z"), Some(1));
    }
}

fn snak_qid(snak: &serde_json::Value) -> Option<String> {
    snak.pointer("/datavalue/value/id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

fn snak_time_year(snak: &serde_json::Value) -> Option<i32> {
    snak.pointer("/datavalue/value/time")
        .and_then(|v| v.as_str())
        .and_then(parse_wd_year)
}

fn claim_year(claim: &serde_json::Value) -> Option<i32> {
    snak_time_year(claim.get("mainsnak")?)
        .or_else(|| {
            claim
                .pointer("/qualifiers/P580/0/datavalue/value/time")
                .and_then(|v| v.as_str())
                .and_then(parse_wd_year)
        })
        .or_else(|| {
            claim
                .pointer("/qualifiers/P585/0/datavalue/value/time")
                .and_then(|v| v.as_str())
                .and_then(parse_wd_year)
        })
}

fn is_military_occupation_qid(qid: &str) -> bool {
    matches!(
        qid,
        "Q47064"
            | "Q189290"
            | "Q4991371"
            | "Q11545923"
            | "Q1402561"
            | "Q11900058"
            | "Q380782"
            | "Q1892901"
            | "Q83307"
            | "Q2304859"
            | "Q1892909"
    )
}

pub(crate) async fn fetch_wikidata_subject_meta(
    qid: &str,
    lang: &str,
) -> anyhow::Result<WikidataSubjectMeta> {
    let client = wiki_http_client()?;
    let resp = send_retrying(
        client.get("https://www.wikidata.org/w/api.php").query(&[
            ("action", "wbgetentities"),
            ("ids", qid),
            ("props", "claims|labels|sitelinks"),
            ("languages", "en"),
            ("format", "json"),
        ]),
        8,
        Duration::from_secs(4),
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e}"))?
    .error_for_status()?
    .json::<serde_json::Value>()
    .await?;
    let entity = resp
        .pointer(&format!("/entities/{qid}"))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("missing wikidata entity {qid}"))?;

    let sitelink_key = format!("{lang}wiki");
    let wiki_title = entity
        .pointer(&format!("/sitelinks/{sitelink_key}/title"))
        .or_else(|| entity.pointer("/sitelinks/enwiki/title"))
        .or_else(|| entity.pointer("/sitelinks/frwiki/title"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let claims = entity.get("claims").cloned().unwrap_or(serde_json::json!({}));
    let birth_year = claims
        .get("P569")
        .and_then(|a| a.as_array())
        .and_then(|a| a.first())
        .and_then(claim_year);
    let death_year = claims
        .get("P570")
        .and_then(|a| a.as_array())
        .and_then(|a| a.first())
        .and_then(claim_year);

    let mut related_qids: Vec<String> = Vec::new();
    let mut occupation_qids: Vec<String> = Vec::new();
    for pid in ["P106", "P39", "P101"] {
        if let Some(arr) = claims.get(pid).and_then(|v| v.as_array()) {
            for stmt in arr {
                if let Some(q) = stmt.get("mainsnak").and_then(snak_qid) {
                    occupation_qids.push(q);
                }
            }
        }
    }
    for pid in [
        "P19", "P20", "P69", "P108", "P551", "P937", "P166", "P800", "P463", "P119", "P27", "P106",
        "P39", "P101",
    ] {
        if let Some(arr) = claims.get(pid).and_then(|v| v.as_array()) {
            for stmt in arr {
                if let Some(q) = stmt.get("mainsnak").and_then(snak_qid) {
                    related_qids.push(q);
                }
            }
        }
    }
    related_qids.sort();
    related_qids.dedup();
    occupation_qids.sort();
    occupation_qids.dedup();

    let mut labels: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut related_titles = Vec::new();
    let mut occupations: Vec<String> = Vec::new();
    if occupation_qids.iter().any(|q| is_military_occupation_qid(q)) {
        occupations.push("military".to_string());
    }
    for pid in ["P607", "P241", "P410", "P1344"] {
        if claims
            .get(pid)
            .and_then(|v| v.as_array())
            .is_some_and(|a| !a.is_empty())
        {
            if !occupations.iter().any(|o| o.eq_ignore_ascii_case("military")) {
                occupations.push("military".to_string());
            }
            break;
        }
    }

    let mut fetch_ids = related_qids.clone();
    fetch_ids.extend(occupation_qids.iter().cloned());
    fetch_ids.sort();
    fetch_ids.dedup();
    for chunk in fetch_ids.chunks(40) {
        let ids = chunk.join("|");
        let extra = match send_retrying(
            client.get("https://www.wikidata.org/w/api.php").query(&[
                ("action", "wbgetentities"),
                ("ids", ids.as_str()),
                ("props", "labels|sitelinks"),
                ("languages", "en"),
                ("format", "json"),
            ]),
            8,
            Duration::from_secs(4),
        )
        .await
        .map_err(|e| anyhow::Error::msg(e.to_string()))
        {
            Ok(resp) => match resp.error_for_status() {
                Ok(ok) => ok.json::<serde_json::Value>().await.ok(),
                Err(e) => {
                    tracing::warn!(error = %e, "wikidata related labels request failed");
                    None
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "wikidata related labels retry exhausted");
                None
            }
        };
        let Some(extra) = extra else {
            continue;
        };
        if let Some(ents) = extra.get("entities").and_then(|v| v.as_object()) {
            for (id, ent) in ents {
                if let Some(lab) = ent
                    .pointer(&format!("/labels/{lang}/value"))
                    .or_else(|| ent.pointer("/labels/en/value"))
                    .and_then(|v| v.as_str())
                {
                    labels.insert(id.clone(), lab.to_string());
                    if occupation_qids.iter().any(|q| q == id)
                        && !occupations
                            .iter()
                            .any(|o: &String| o.eq_ignore_ascii_case(lab))
                    {
                        occupations.push(lab.to_string());
                    }
                }
                if let Some(t) = ent
                    .pointer(&format!("/sitelinks/{sitelink_key}/title"))
                    .or_else(|| ent.pointer("/sitelinks/enwiki/title"))
                    .or_else(|| ent.pointer("/sitelinks/frwiki/title"))
                    .and_then(|v| v.as_str())
                {
                    related_titles.push(t.to_string());
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }

    let label_of = |pid: &str| -> Option<String> {
        claims
            .get(pid)
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .and_then(|s| s.get("mainsnak"))
            .and_then(snak_qid)
            .and_then(|q| labels.get(&q).cloned())
    };

    let mut lines = Vec::new();
    if let Some(y) = birth_year {
        let place = label_of("P19").unwrap_or_default();
        lines.push(format!("STATEMENT\tbirth\tborn_in\t{y}\t{place}"));
    }
    if let Some(y) = death_year {
        let place = label_of("P20").unwrap_or_default();
        lines.push(format!("STATEMENT\tdeath\tdied_in\t{y}\t{place}"));
    }
    const MAP: &[(&str, &str, &str)] = &[
        ("P26", "marriage", "married"),
        ("P39", "office", "held_office"),
        ("P69", "education", "studied_at"),
        ("P108", "office", "worked_at"),
        ("P551", "residence", "resided_in"),
        ("P937", "residence", "worked_in"),
        ("P166", "award", "awarded"),
        ("P800", "publication", "created"),
        ("P101", "office", "field_of_work"),
        ("P119", "burial", "buried_at"),
    ];
    for (pid, etype, pred) in MAP {
        let Some(arr) = claims.get(*pid).and_then(|v| v.as_array()) else {
            continue;
        };
        for stmt in arr.iter().take(12) {
            let q = stmt.get("mainsnak").and_then(snak_qid);
            let place = q.as_ref().and_then(|id| labels.get(id)).cloned().unwrap_or_default();
            let year = claim_year(stmt)
                .map(|y| y.to_string())
                .unwrap_or_default();
            if year.is_empty() && place.is_empty() {
                continue;
            }
            lines.push(format!("STATEMENT\t{etype}\t{pred}\t{year}\t{place}"));
        }
    }

    Ok(WikidataSubjectMeta {
        birth_year,
        death_year,
        occupations,
        wiki_title,
        related_titles,
        statements_text: lines.join("\n"),
    })
}

async fn fetch_wikipedia_article_links(
    lang: &str,
    title: &str,
    limit: usize,
) -> anyhow::Result<Vec<String>> {
    let client = wiki_http_client()?;
    let api = format!("https://{lang}.wikipedia.org/w/api.php");
    let mut out = Vec::new();
    let mut plcontinue: Option<String> = None;
    for _ in 0..8 {
        let mut req = client.get(&api).query(&[
            ("action", "query"),
            ("prop", "links"),
            ("titles", title),
            ("plnamespace", "0"),
            ("pllimit", "500"),
            ("format", "json"),
            ("redirects", "1"),
        ]);
        if let Some(c) = &plcontinue {
            req = req.query(&[("plcontinue", c.as_str())]);
        }
        let resp = send_retrying(req, 8, Duration::from_secs(4))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        if let Some(pages) = resp.pointer("/query/pages").and_then(|v| v.as_object()) {
            for page in pages.values() {
                if let Some(links) = page.get("links").and_then(|v| v.as_array()) {
                    for link in links {
                        if let Some(t) = link.get("title").and_then(|v| v.as_str()) {
                            out.push(t.to_string());
                        }
                    }
                }
            }
        }
        plcontinue = resp
            .pointer("/continue/plcontinue")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        if plcontinue.is_none() || out.len() >= limit {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    out.truncate(limit);
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
async fn ingest_memory_document(
    pool: &sqlx::PgPool,
    config: &AppConfig,
    subject_res: &ResolvedSubject,
    subject_id: Uuid,
    subject: &str,
    source_type: &str,
    source_uri: &str,
    document_type: &str,
    text: &str,
    page_coords: Option<(f64, f64)>,
    extractor_refs: &[&dyn CandidateExtractor],
    resolver: &GazetteerResolver,
    projections: &DerivedLabelProjections,
    metrics: &mut LotEMetrics,
) -> anyhow::Result<()> {
    let hash = content_hash(text);
    let snapshot_id = insert_document_snapshot(
        pool,
        &DocumentSnapshotInsert {
            source_type: source_type.into(),
            source_uri: source_uri.into(),
            source_identifier: Some(source_uri.into()),
            language: config.wiki_lang.clone(),
            title: Some(subject.into()),
            content_hash: hash,
            revision_id: None,
            wiki_page_id: None,
            raw_document_id: None,
            text: text.to_string(),
            metadata: serde_json::json!({ "lot": "E", "memory": true }),
        },
    )
    .await?;
    let frag_id = insert_document_fragment(
        pool,
        &DocumentFragmentInsert {
            snapshot_id,
            fragment_kind: "sentence".into(),
            parent_fragment_id: None,
            sentence_id: None,
            text: text.to_string(),
            start_offset: 0,
            end_offset: text.len() as i32,
            clause_index: None,
            ordinal: 0,
        },
    )
    .await?;
    let input = ExtractorInput {
        text: text.to_string(),
        page_title: Some(subject.into()),
        subject_label: Some(subject.into()),
        document_type: document_type.into(),
        subject_death_year: subject_res.death_year,
    };
    let mut raws = Vec::new();
    for ex in extractor_refs {
        raws.extend(ex.extract(&input));
    }
    for raw in raws {
        process_one(
            pool,
            config,
            subject_res,
            subject_id,
            snapshot_id,
            frag_id,
            &raw,
            resolver,
            projections,
            page_coords,
            metrics,
        )
        .await?;
    }
    Ok(())
}

pub fn write_minimal_seed_list(subject: &str) -> anyhow::Result<PathBuf> {
    let slug: String = subject
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let slug = slug.trim_matches('_');
    let dir = Path::new("/tmp/talaria_seeds");
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{slug}_wiki_titles.txt"));
    std::fs::write(&path, format!("# auto seed for {subject}\n{subject}\n"))?;
    Ok(path)
}

pub fn default_napoleon_seed() -> PathBuf {
    PathBuf::from("fixtures/seeds/napoleon_wiki_titles.txt")
}

pub fn connector_status_json() -> String {
    let europeana = std::env::var("EUROPEANA_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
        .map(|_| "extraction_ready")
        .unwrap_or("needs_EUROPEANA_API_KEY");
    serde_json::to_string_pretty(&serde_json::json!({
        "wikipedia": "extraction_ready",
        "wikidata": "fetch_ready",
        "wikisource": "extraction_ready",
        "commons": "extraction_ready",
        "fixture": "production_ready",
        "bnf": "extraction_ready",
        "gallica": "extraction_ready",
        "persee": "extraction_ready",
        "hal": "extraction_ready",
        "theses_fr": "extraction_ready",
        "idref": "stub",
        "sudoc": "stub",
        "archives_nationales": "stub",
        "open_library": "extraction_ready",
        "internet_archive": "extraction_ready",
        "europeana": europeana,
        "loc": "stub",
        "viaf": "metadata_only",
        "isni": "metadata_only",
        "openalex": "extraction_ready",
        "crossref": "stub",
        "note": "Executable with --live from explorer search or ingest-quality: wikipedia, wikidata, wikisource, commons, hal, persee, gallica, theses_fr, open_library, open_alex, internet_archive, bnf. Europeana needs EUROPEANA_API_KEY."
    }))
    .unwrap_or_else(|_| "{}".into())
}

pub async fn run_resolve_places(
    config: &AppConfig,
    subject: &str,
    _all_unresolved: bool,
) -> anyhow::Result<String> {
    let (pool, subject_id) = open_db_for_subject(config, subject, "person").await?;

    let unresolved: Vec<(Uuid, Option<String>)> = sqlx::query_as(
        r#"
        SELECT id, place_label FROM canonical_events
        WHERE pipeline = 'quality' AND is_active AND timeline_eligible AND NOT map_eligible
          AND entity_id = $1 AND place_label IS NOT NULL
        "#,
    )
    .bind(subject_id)
    .fetch_all(&pool)
    .await?;

    let mut by_label: HashMap<String, Vec<Uuid>> = HashMap::new();
    for (eid, label) in unresolved {
        let Some(label) = label.filter(|s| !s.trim().is_empty()) else {
            continue;
        };
        by_label.entry(label).or_default().push(eid);
    }

    let unique_labels: Vec<String> = by_label.keys().cloned().collect();
    let unique_n = unique_labels.len();
    // Wikidata rate-limits aggressive parallelism (HTTP 429). One inflight lookup.
    let sem = Arc::new(tokio::sync::Semaphore::new(1));
    let mut join = tokio::task::JoinSet::new();
    for label in unique_labels {
        let sem = sem.clone();
        join.spawn(async move {
            let _permit = sem.acquire_owned().await.ok();
            let hit = resolve_label_coords(&label).await;
            (label, hit)
        });
    }

    let mut resolved = 0u32;
    let mut failed = 0u32;
    let mut samples = Vec::new();
    while let Some(joined) = join.join_next().await {
        let Ok((label, hit)) = joined else {
            failed += 1;
            continue;
        };
        let Some(ids) = by_label.get(&label) else {
            continue;
        };
        if let Some(hit) = hit {
            for eid in ids {
                apply_place_to_quality_event(
                    &pool,
                    *eid,
                    &label,
                    None,
                    hit.lat,
                    hit.lon,
                    &hit.precision,
                    hit.uncertainty,
                )
                .await?;
                resolved += 1;
            }
        } else {
            failed += ids.len() as u32;
            if samples.len() < 25 {
                samples.push(label);
            }
        }
    }

    let density = density_report_counts(&pool, Some(subject_id)).await?;
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "subject": subject,
        "attempted": resolved + failed,
        "unique_labels": unique_n,
        "resolved": resolved,
        "still_unresolved": failed,
        "unresolved_samples": samples,
        "map_eligible": density.map_eligible,
        "timeline_eligible": density.timeline_eligible,
        "events_without_place": density.events_without_place,
    }))?)
}

struct PlaceHit {
    lat: f64,
    lon: f64,
    precision: String,
    uncertainty: Option<f64>,
}

async fn resolve_label_coords(label: &str) -> Option<PlaceHit> {
    if let Some(res) = resolve_place_offline(label) {
        return Some(PlaceHit {
            lat: res.lat,
            lon: res.lon,
            precision: res.precision,
            uncertainty: res.uncertainty_radius_m,
        });
    }
    if let Some(hint) = place_hint_from_title(label) {
        if let Some(res) = resolve_place_offline(&hint) {
            return Some(PlaceHit {
                lat: res.lat,
                lon: res.lon,
                precision: res.precision,
                uncertainty: res.uncertainty_radius_m,
            });
        }
    }
    if let Some((lat, lon)) = fetch_wikidata_coords_for_label(label).await {
        tokio::time::sleep(Duration::from_millis(250)).await;
        return Some(PlaceHit {
            lat,
            lon,
            precision: "wikidata_p625".into(),
            uncertainty: Some(5000.0),
        });
    }
    if let Some(hint) = place_hint_from_title(label) {
        if hint != label {
            if let Some((lat, lon)) = fetch_wikidata_coords_for_label(&hint).await {
                tokio::time::sleep(Duration::from_millis(250)).await;
                return Some(PlaceHit {
                    lat,
                    lon,
                    precision: "wikidata_p625".into(),
                    uncertainty: Some(5000.0),
                });
            }
        }
    }
    None
}

async fn fetch_wikidata_coords_for_label(label: &str) -> Option<(f64, f64)> {
    if !is_plausible_place_label(label) {
        return None;
    }
    let client = reqwest::Client::builder()
        .user_agent("TalariaEngine/0.1 (resolve-places; https://www.wikidata.org/wiki/Wikidata:Data_access)")
        .timeout(Duration::from_secs(12))
        .build()
        .ok()?;
    let search = client
        .get("https://www.wikidata.org/w/api.php")
        .query(&[
            ("action", "wbsearchentities"),
            ("search", label),
            ("language", "en"),
            ("limit", "5"),
            ("format", "json"),
            ("type", "item"),
        ])
        .send()
        .await
        .ok()?
        .json::<serde_json::Value>()
        .await
        .ok()?;
    let ids: Vec<String> = search
        .get("search")?
        .as_array()?
        .iter()
        .filter_map(|h| h.get("id").and_then(|v| v.as_str()).map(str::to_string))
        .collect();
    if ids.is_empty() {
        return None;
    }
    let ids_joined = ids.join("|");
    let entity = client
        .get("https://www.wikidata.org/w/api.php")
        .query(&[
            ("action", "wbgetentities"),
            ("ids", ids_joined.as_str()),
            ("props", "claims"),
            ("format", "json"),
        ])
        .send()
        .await
        .ok()?
        .json::<serde_json::Value>()
        .await
        .ok()?;
    for qid in &ids {
        let lat = entity
            .pointer(&format!(
                "/entities/{qid}/claims/P625/0/mainsnak/datavalue/value/latitude"
            ))
            .and_then(|v| v.as_f64());
        let lon = entity
            .pointer(&format!(
                "/entities/{qid}/claims/P625/0/mainsnak/datavalue/value/longitude"
            ))
            .and_then(|v| v.as_f64());
        if let (Some(lat), Some(lon)) = (lat, lon) {
            return Some((lat, lon));
        }
    }
    None
}

/// Soft-deactivate quality events whose place_label is clearly non-geographic noise.
pub async fn deactivate_implausible_place_events(
    config: &AppConfig,
    subject: &str,
) -> anyhow::Result<u64> {
    let pool = connect(config).await?;
    let subject_id = upsert_entity_with_kind(&pool, &config.wiki_lang, subject, "person").await?;
    let rows: Vec<(Uuid, Option<String>)> = sqlx::query_as(
        r#"
        SELECT id, place_label FROM canonical_events
        WHERE pipeline = 'quality' AND is_active AND entity_id = $1
          AND place_label IS NOT NULL
        "#,
    )
    .bind(subject_id)
    .fetch_all(&pool)
    .await?;
    let mut n = 0u64;
    for (id, label) in rows {
        let Some(label) = label else { continue };
        if !is_plausible_place_label(&label) {
            sqlx::query(
                r#"
                UPDATE canonical_events SET is_active = false
                WHERE id = $1 AND pipeline = 'quality'
                "#,
            )
            .bind(id)
            .execute(&pool)
            .await?;
            n += 1;
        }
    }
    Ok(n)
}

pub async fn run_density_report(
    config: &AppConfig,
    subject: Option<&str>,
    show_bottlenecks: bool,
    show_source_coverage: bool,
    show_unresolved_places: bool,
) -> anyhow::Result<String> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let sid = if let Some(label) = subject {
        Some(upsert_entity_with_kind(&pool, &config.wiki_lang, label, "person").await?)
    } else {
        None
    };
    let counts = density_report_counts(&pool, sid).await?;

    let mut report = serde_json::json!({
        "documents_discovered": counts.documents_discovered,
        "documents_snapshotted": counts.documents_snapshotted,
        "fragments": counts.fragments,
        "candidates": counts.candidates,
        "rejected": counts.rejected,
        "needs_review": counts.needs_review,
        "claims": counts.claims,
        "accepted_events": counts.accepted_events,
        "timeline_eligible": counts.timeline_eligible,
        "map_eligible": counts.map_eligible,
        "events_without_place": counts.events_without_place,
        "multi_source_events": counts.multi_source_events,
        "targets": {
            "timeline": 500,
            "map": 500,
            "gap_timeline": 500i64.saturating_sub(counts.timeline_eligible),
            "gap_map": 500i64.saturating_sub(counts.map_eligible),
            "status": if counts.map_eligible >= 500 && counts.timeline_eligible >= 500 {
                "target_reached"
            } else {
                "target_not_reached"
            }
        }
    });

    if show_bottlenecks {
        let reasons: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT code, COUNT(*)::bigint FROM (
              SELECT jsonb_array_elements_text(rejection_codes) AS code
              FROM event_candidates
              WHERE status = 'rejected'
                AND ($1::uuid IS NULL OR subject_entity_id = $1)
            ) t
            GROUP BY code ORDER BY COUNT(*) DESC LIMIT 20
            "#,
        )
        .bind(sid)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
        report["bottlenecks"] = serde_json::json!({
            "rejection_codes": reasons.iter().map(|(c,n)| serde_json::json!({"code": c, "n": n})).collect::<Vec<_>>(),
            "primary": if counts.documents_snapshotted < 50 {
                "insufficient_documents"
            } else if counts.candidates > 0 && counts.accepted_events * 3 < counts.candidates {
                "gates_or_dedupe"
            } else if counts.timeline_eligible > counts.map_eligible + 10 {
                "unresolved_places"
            } else {
                "exploration_or_extractors"
            }
        });
    }

    if show_source_coverage {
        report["connectors"] = serde_json::from_str(&connector_status_json()).unwrap_or_default();
        let by_source: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT source_type, COUNT(*)::bigint FROM document_snapshots
            GROUP BY source_type ORDER BY COUNT(*) DESC
            "#,
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
        report["snapshots_by_source"] = serde_json::json!(by_source
            .iter()
            .map(|(s, n)| serde_json::json!({"source": s, "n": n}))
            .collect::<Vec<_>>());
    }

    if show_unresolved_places {
        let places: Vec<(String, i64)> = if let Some(id) = sid {
            sqlx::query_as(
                r#"
                SELECT COALESCE(place_label, '(null)'), COUNT(*)::bigint
                FROM canonical_events
                WHERE pipeline = 'quality' AND is_active AND timeline_eligible AND NOT map_eligible
                  AND entity_id = $1
                GROUP BY 1 ORDER BY COUNT(*) DESC LIMIT 40
                "#,
            )
            .bind(id)
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
        } else {
            Vec::new()
        };
        report["unresolved_places"] = serde_json::json!(places
            .iter()
            .map(|(l, n)| serde_json::json!({"label": l, "n": n}))
            .collect::<Vec<_>>());
    }

    Ok(serde_json::to_string_pretty(&report)?)
}

pub async fn run_exploration_report(config: &AppConfig, subject: &str) -> anyhow::Result<String> {
    let (pool, subject_id) = open_db_for_subject(config, subject, "person").await?;
    let queue: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT status, COUNT(*)::bigint FROM exploration_targets
        WHERE subject_entity_id = $1
        GROUP BY status
        "#,
    )
    .bind(subject_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let seed_path = default_napoleon_seed();
    let seed_n = load_seed_titles(&seed_path).map(|t| t.len()).unwrap_or(0);
    let wiki_snaps: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM document_snapshots WHERE source_type = 'wikipedia'"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "subject": subject,
        "seed_titles_available": seed_n,
        "wikipedia_snapshots": wiki_snaps,
        "exploration_queue_by_status": queue.iter().map(|(s,n)| serde_json::json!({"status": s, "n": n})).collect::<Vec<_>>(),
        "note": "Lot E primarily drives exploration from fixtures/seeds/napoleon_wiki_titles.txt; exploration_targets table is ready for resume/queue."
    }))?)
}
