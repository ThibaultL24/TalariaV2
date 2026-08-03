// crates/talaria-api/src/lot_e.rs
//! Lot E: dense seed-driven quality ingest toward density targets.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use talaria_core::AppConfig;
use talaria_judge::parse_time_surface;
use talaria_quality::{
    apply_gates, occurrence_key, parse_typed_time, resolve_mentions, BuildProjections,
    DerivedLabelProjections, EntityKind, EvidencePtr, GazetteerResolver, GateContext, TypedTime,
    ASSEMBLER_V1,
};
use talaria_sources::extractors::{
    claim_fingerprint, default_extractor_stack, CandidateExtractor, ClaimKey, ExtractorInput,
};
use talaria_sources::{
    is_plausible_place_label, load_seed_titles, place_hint_from_title, resolve_place_offline,
    DensityProgress, DensityTargets, ResolvedSubject,
};
use talaria_store::{
    add_claim_support, apply_place_to_quality_event, connect, density_report_counts,
    find_active_quality_event_by_fingerprint, find_active_singleton, insert_document_fragment,
    insert_document_snapshot, insert_quality_canonical_event, link_claim_to_event,
    mark_candidate_assembled, quality_lifespan_years, reinforce_quality_event, run_migrations,
    update_event_candidate_judgment, upsert_entity_with_kind, upsert_event_candidate,
    upsert_quality_claim, DocumentFragmentInsert, DocumentSnapshotInsert, EventCandidateInsert,
    QualityClaimInsert, QualityEventInsert,
};
use uuid::Uuid;

fn content_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

fn time_to_json(time: &TypedTime) -> serde_json::Value {
    serde_json::to_value(time).unwrap_or_else(|_| serde_json::json!({"kind":"unknown"}))
}

fn start_time_from_typed(time: &TypedTime) -> Option<chrono::DateTime<chrono::Utc>> {
    match time {
        TypedTime::Exact {
            year,
            month,
            day,
            ..
        } => {
            let m = month.unwrap_or(6);
            let d = day.unwrap_or(15);
            chrono::NaiveDate::from_ymd_opt(*year, m, d)
                .and_then(|nd| nd.and_hms_opt(0, 0, 0))
                .map(|n| chrono::DateTime::from_naive_utc_and_offset(n, chrono::Utc))
        }
        _ => time
            .year_for_gates()
            .and_then(|y| parse_time_surface(&y.to_string()).map(|p| p.start)),
    }
}

async fn fetch_wikipedia_extract(
    lang: &str,
    title: &str,
) -> anyhow::Result<(String, String, Option<String>, Option<(f64, f64)>)> {
    let client = reqwest::Client::builder()
        .user_agent("TalariaEngine/0.1 (lot-e density; research)")
        .timeout(Duration::from_secs(45))
        .build()?;
    let api = format!("https://{lang}.wikipedia.org/w/api.php");
    let response = client
        .get(&api)
        .query(&[
            ("action", "query"),
            ("prop", "extracts|info|coordinates"),
            ("explaintext", "1"),
            ("exlimit", "1"),
            ("titles", title),
            ("format", "json"),
            ("redirects", "1"),
            ("colimit", "1"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
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
    if extract.is_empty() {
        anyhow::bail!("empty extract for {title}");
    }
    Ok((resolved, extract, revid, coords))
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
    let pool = connect(config).await?;
    run_migrations(&pool).await?;

    let subject_id =
        upsert_entity_with_kind(&pool, &config.wiki_lang, subject, "person").await?;
    let (by, dy, _, _) = quality_lifespan_years(&pool, subject_id).await?;
    let mut subject_res = ResolvedSubject {
        entity_id: Some(subject_id),
        qid: qid.map(str::to_string),
        label: subject.into(),
        languages: vec![lang.into(), "en".into(), "fr".into()],
        birth_year: by.or(Some(1769)),
        death_year: dy.or(Some(1821)),
        countries: vec!["France".into()],
        occupations: vec!["military".into()],
        known_identifiers: vec![],
    };

    let mut titles = load_seed_titles(seed_list)?;
    if let Some(max) = max_titles {
        titles.truncate(max as usize);
    }

    // Soft-clean implausible place noise from earlier extractor versions before counting.
    let cleaned = deactivate_implausible_place_events(config, subject).await.unwrap_or(0);
    if cleaned > 0 {
        tracing::info!(cleaned, "deactivated quality events with implausible place labels");
    }

    let extractors = default_extractor_stack();
    let extractor_refs: Vec<&dyn CandidateExtractor> =
        extractors.iter().map(|e| e.as_ref()).collect();
    let resolver = GazetteerResolver;
    let projections = DerivedLabelProjections;
    let mut metrics = LotEMetrics::default();

    let original_seeds: Vec<String> = titles.clone();
    let mut title_queue: std::collections::VecDeque<String> =
        titles.into_iter().collect();
    let mut seen_titles: std::collections::HashSet<String> =
        title_queue.iter().cloned().collect();
    let langs = if lang == "en" {
        vec![lang.to_string(), "fr".to_string()]
    } else {
        vec![lang.to_string()]
    };

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
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Grow exploration queue from high-value linked titles mentioned in the extract.
        for linked in discover_linked_titles(&text) {
            if seen_titles.len() < targets.max_linked_entities as usize
                && seen_titles.insert(linked.clone())
            {
                title_queue.push_back(linked);
            }
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

        // Refresh lifespan after birth/death accepted
        let (by2, dy2, _, _) = quality_lifespan_years(&pool, subject_id).await?;
        subject_res.birth_year = by2.or(subject_res.birth_year);
        subject_res.death_year = dy2.or(subject_res.death_year);

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

        // Ensure battle/siege pages produce at least a page-level candidate if extractors missed.
        if !raws.iter().any(|r| {
            r.extractor_id == "military_campaign"
                && r.object_surface.as_deref() == Some(resolved_title.as_str())
        }) {
            if let Some(place) = place_hint_from_title(&resolved_title) {
                if let Some(year) = first_year_in(&text) {
                    raws.push(talaria_sources::extractors::RawCandidate {
                        event_type: if resolved_title.to_lowercase().starts_with("siege")
                            || resolved_title.to_lowercase().starts_with("siège")
                        {
                            "siege".into()
                        } else if resolved_title.to_lowercase().starts_with("treaty")
                            || resolved_title.to_lowercase().starts_with("traité")
                        {
                            "treaty".into()
                        } else {
                            "battle".into()
                        },
                        predicate: "fought_at".into(),
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
            "fixture": "production_ready",
            "bnf": "stub",
            "gallica": "stub",
            "europeana": "stub",
            "open_library": "stub",
            "internet_archive": "stub",
        }
    });
    let s = serde_json::to_string_pretty(&report)?;
    println!("{s}");
    Ok(s)
}

fn first_year_in(text: &str) -> Option<String> {
    for w in text.split(|c: char| !c.is_ascii_digit()) {
        if w.len() == 4 {
            if let Ok(y) = w.parse::<i32>() {
                if (1700..=1900).contains(&y) {
                    return Some(y.to_string());
                }
            }
        }
    }
    None
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
    if let Some(ref pl) = shell.place_label {
        if let Some(pres) = resolve_place_offline(pl) {
            lat = Some(pres.lat);
            lon = Some(pres.lon);
            location_precision = Some(pres.precision.clone());
            uncertainty = pres.uncertainty_radius_m;
            shell.place_entity_id = Some(
                upsert_entity_with_kind(pool, &config.wiki_lang, pl, "place").await?,
            );
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
                shell.place_entity_id = Some(
                    upsert_entity_with_kind(pool, &config.wiki_lang, pl, "place").await?,
                );
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
                shell.place_entity_id = Some(
                    upsert_entity_with_kind(pool, &config.wiki_lang, &hint, "place").await?,
                );
            }
        }
    }

    let occ = occurrence_key(
        &subject.label,
        &shell.event_type,
        &shell.predicate,
        &shell.time,
        shell.place_label.as_deref(),
        raw.object_surface.as_deref(),
        Some(&raw.extractor_id),
        None,
    );
    shell.fingerprint = occ.clone();

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
            status: "pending".into(),
            rejection_codes: vec![],
            judgment_json: serde_json::json!({"occurrence_key": occ, "primary_object": raw.object_surface}),
        },
    )
    .await?;
    if !inserted {
        return Ok(());
    }
    metrics.candidates += 1;

    let (birth_year, death_year, has_birth, has_death) =
        quality_lifespan_years(pool, subject_id).await?;
    let ctx = GateContext {
        subject_birth_year: birth_year.or(subject.birth_year),
        subject_death_year: death_year.or(subject.death_year),
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

    if let Some(existing) = find_active_quality_event_by_fingerprint(pool, &occ).await? {
        reinforce_quality_event(pool, existing).await?;
        mark_candidate_assembled(pool, cand_id, existing).await?;
        link_claim_to_event(pool, claim_id, existing).await?;
        metrics.events_reinforced += 1;
        return Ok(());
    }

    if shell.event_type == "birth" || shell.event_type == "death" {
        if find_active_singleton(pool, subject_id, &shell.event_type)
            .await?
            .is_some()
        {
            update_event_candidate_judgment(
                pool,
                cand_id,
                "rejected",
                &["singleton_cardinality_violation".into()],
                &serde_json::json!({"at":"assemble"}),
                shell.subject_entity_id,
                shell.place_entity_id,
                shell.place_label.as_deref(),
                &serde_json::json!([]),
                &serde_json::json!([]),
                &serde_json::json!([]),
            )
            .await?;
            metrics.rejected += 1;
            metrics.accepted = metrics.accepted.saturating_sub(1);
            metrics.bump("singleton_cardinality_violation");
            return Ok(());
        }
    }

    let map_eligible = lat.is_some() && lon.is_some();
    let proj = projections.from_candidate(&shell, &subject.label);
    let title_derived = projections.display_label(&proj);

    let event_id = insert_quality_canonical_event(
        pool,
        &QualityEventInsert {
            entity_id: subject_id,
            event_type: shell.event_type.clone(),
            epistemic_status: "attested".into(),
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
            fingerprint: occ,
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

pub fn default_napoleon_seed() -> PathBuf {
    PathBuf::from("fixtures/seeds/napoleon_wiki_titles.txt")
}

pub fn connector_status_json() -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "wikipedia": "extraction_ready",
        "wikidata": "fetch_ready",
        "wikisource": "stub",
        "commons": "stub",
        "fixture": "production_ready",
        "bnf": "stub",
        "gallica": "stub",
        "persee": "stub",
        "idref": "stub",
        "sudoc": "stub",
        "archives_nationales": "stub",
        "open_library": "stub",
        "internet_archive": "stub",
        "europeana": "stub",
        "loc": "stub",
        "viaf": "metadata_only",
        "isni": "metadata_only",
        "openalex": "stub",
        "crossref": "stub",
        "note": "Only wikipedia/wikidata/fixture are executable beyond stubs; stubs must not be reported as integrated."
    }))
    .unwrap_or_else(|_| "{}".into())
}

pub async fn run_resolve_places(
    config: &AppConfig,
    subject: &str,
    _all_unresolved: bool,
) -> anyhow::Result<String> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let subject_id =
        upsert_entity_with_kind(&pool, &config.wiki_lang, subject, "person").await?;

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

    let mut resolved = 0u32;
    let mut failed = 0u32;
    let mut samples = Vec::new();
    for (eid, label) in &unresolved {
        let Some(label) = label else {
            continue;
        };
        if let Some(res) = resolve_place_offline(label) {
            apply_place_to_quality_event(
                &pool,
                *eid,
                label,
                None,
                res.lat,
                res.lon,
                &res.precision,
                res.uncertainty_radius_m,
            )
            .await?;
            resolved += 1;
        } else if let Some(res) = fetch_wikidata_coords_for_label(label).await {
            apply_place_to_quality_event(
                &pool,
                *eid,
                label,
                None,
                res.0,
                res.1,
                "wikidata_p625",
                Some(5000.0),
            )
            .await?;
            resolved += 1;
            tokio::time::sleep(Duration::from_millis(200)).await;
        } else {
            failed += 1;
            if samples.len() < 25 {
                samples.push(label.clone());
            }
        }
    }

    let density = density_report_counts(&pool, Some(subject_id)).await?;
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "subject": subject,
        "attempted": unresolved.len(),
        "resolved": resolved,
        "still_unresolved": failed,
        "unresolved_samples": samples,
        "map_eligible": density.map_eligible,
        "timeline_eligible": density.timeline_eligible,
        "events_without_place": density.events_without_place,
    }))?)
}

async fn fetch_wikidata_coords_for_label(label: &str) -> Option<(f64, f64)> {
    if !is_plausible_place_label(label) {
        return None;
    }
    let client = reqwest::Client::builder()
        .user_agent("TalariaEngine/0.1 (lot-e resolve-places)")
        .timeout(Duration::from_secs(30))
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
            .pointer(&format!("/entities/{qid}/claims/P625/0/mainsnak/datavalue/value/latitude"))
            .and_then(|v| v.as_f64());
        let lon = entity
            .pointer(&format!("/entities/{qid}/claims/P625/0/mainsnak/datavalue/value/longitude"))
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
    let subject_id =
        upsert_entity_with_kind(&pool, &config.wiki_lang, subject, "person").await?;
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
        report["snapshots_by_source"] = serde_json::json!(
            by_source.iter().map(|(s,n)| serde_json::json!({"source": s, "n": n})).collect::<Vec<_>>()
        );
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
        report["unresolved_places"] = serde_json::json!(
            places.iter().map(|(l,n)| serde_json::json!({"label": l, "n": n})).collect::<Vec<_>>()
        );
    }

    Ok(serde_json::to_string_pretty(&report)?)
}

pub async fn run_exploration_report(config: &AppConfig, subject: &str) -> anyhow::Result<String> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let subject_id =
        upsert_entity_with_kind(&pool, &config.wiki_lang, subject, "person").await?;
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
