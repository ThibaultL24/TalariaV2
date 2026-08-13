// crates/talaria-api/src/ingest.rs
//! Multi-source quality ingest orchestration (Lot A/B).

use sha2::{Digest, Sha256};
use talaria_core::AppConfig;
use talaria_judge::{parse_place_surface, parse_time_surface};
use talaria_quality::{
    apply_gates, candidate_fingerprint, existing_candidate_action, occurrence_key_for_event,
    occurrence_stem_for_event, parse_typed_time, resolve_mentions, should_reinforce_existing_event,
    BuildProjections, DerivedLabelProjections, EntityKind, EvidencePtr, ExistingCandidateAction,
    EXTRACTOR_EPISTEMIC_STATUS, GazetteerResolver, GateContext, Mention, TypedTime, ASSEMBLER_V1,
};
use talaria_sources::connectors::{default_registry, FixtureConnector};
use talaria_sources::extractors::{
    claim_fingerprint, default_extractor_stack, CandidateExtractor, ClaimKey, ExtractorInput,
    StructuredStatementExtractor,
};
use talaria_sources::wdqs::{
    events_from_fixture_dir, events_to_statement_text, fetch_events_for_person,
};
use talaria_sources::{
    plan_sources, BudgetCounters, DiscoveredDocument, IngestBudgets, ResolvedSubject, SourceKind,
};
use talaria_store::{
    add_claim_support, connect, density_report_counts,
    find_active_quality_event_by_occurrence_key, find_active_singleton, finish_discovery_run,
    get_event_candidate_by_fingerprint, insert_document_fragment, insert_document_snapshot,
    insert_quality_canonical_event, link_claim_to_event, mark_candidate_assembled,
    mark_discovered_skipped, mark_discovered_snapshotted, quality_lifespan_years,
    reinforce_quality_event, run_migrations, start_discovery_run, update_event_candidate_judgment,
    upsert_discovered_document, upsert_entity_with_kind, upsert_event_candidate,
    upsert_quality_claim, DiscoveredDocumentInsert, DiscoveryRunInsert, DocumentFragmentInsert,
    DocumentSnapshotInsert, EventCandidateInsert, QualityClaimInsert, QualityEventInsert,
};
use uuid::Uuid;

use crate::place_conflict::{abstain_if_competing_place, competing_place_codes};

fn content_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

fn time_to_json(time: &TypedTime) -> serde_json::Value {
    serde_json::to_value(time).unwrap_or_else(|_| serde_json::json!({"kind":"unknown"}))
}

fn start_time_from_typed(time: &TypedTime) -> Option<chrono::DateTime<chrono::Utc>> {
    time.year_for_gates()
        .and_then(|y| parse_time_surface(&y.to_string()).map(|p| p.start))
}

#[derive(Debug, Default, Clone)]
pub struct IngestMetrics {
    pub documents_discovered: u64,
    pub documents_snapshotted: u64,
    pub documents_skipped: u64,
    pub fragments: u64,
    pub candidates: u64,
    pub candidates_deduped: u64,
    pub accepted: u64,
    pub rejected: u64,
    pub needs_review: u64,
    pub claims_created: u64,
    pub claims_reinforced: u64,
    pub events_created: u64,
    pub events_reinforced: u64,
    pub connector_errors: u64,
    pub loss_reasons: std::collections::BTreeMap<String, u64>,
}

impl IngestMetrics {
    fn bump_loss(&mut self, reason: &str) {
        *self.loss_reasons.entry(reason.to_string()).or_insert(0) += 1;
    }
}

pub async fn run_source_registry(live: bool) -> anyhow::Result<()> {
    let reg = default_registry(None, live)?;
    for e in reg.list() {
        println!(
            "{}\timplemented={}\t{}",
            e.kind.as_str(),
            e.implemented,
            e.config_notes
        );
    }
    Ok(())
}

pub async fn run_plan_sources(label: &str, qid: Option<&str>) -> anyhow::Result<()> {
    let subject = ResolvedSubject {
        entity_id: None,
        qid: qid.map(str::to_string),
        label: label.into(),
        languages: vec!["fr".into(), "en".into()],
        birth_year: None,
        death_year: None,
        countries: vec!["France".into()],
        occupations: vec!["military".into(), "statesman".into()],
        known_identifiers: qid
            .map(|q| vec![("wikidata".into(), q.to_string())])
            .unwrap_or_default(),
    };
    let plan = plan_sources(&subject, IngestBudgets::default());
    println!("{}", serde_json::to_string_pretty(&plan)?);
    Ok(())
}

pub async fn run_ingest_quality(
    config: &AppConfig,
    label: &str,
    qid: Option<&str>,
    sources_filter: Option<Vec<String>>,
    use_fixture: bool,
    live: bool,
) -> anyhow::Result<String> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;

    let subject_id = upsert_entity_with_kind(&pool, &config.wiki_lang, label, "person").await?;

    let mut subject = ResolvedSubject {
        entity_id: Some(subject_id),
        qid: qid.map(str::to_string),
        label: label.into(),
        languages: vec!["fr".into(), "en".into()],
        birth_year: None,
        death_year: None,
        countries: vec!["France".into()],
        occupations: vec!["military".into()],
        known_identifiers: vec![],
    };
    let (by, dy, _, _) = quality_lifespan_years(&pool, subject_id).await?;
    subject.birth_year = by;
    subject.death_year = dy;

    if live {
        if let Some(qid) = subject.qid.clone() {
            match ingest_wdqs_events(&pool, config, &subject, subject_id).await {
                Ok(wdqs_metrics) => {
                    tracing::info!(
                        created = wdqs_metrics.events_created,
                        accepted = wdqs_metrics.accepted,
                        "WDQS events ingested"
                    );
                }
                Err(e) => tracing::error!(error = %e, %qid, "WDQS ingest failed"),
            }
        }
    }

    let budgets = IngestBudgets {
        max_documents_per_source: 25,
        max_external_calls: 100,
        min_relevance: 0.35,
        ..IngestBudgets::default()
    };
    let plan = plan_sources(&subject, budgets.clone());

    let fixture = if use_fixture {
        Some(FixtureConnector::dense_biography_pack(label))
    } else {
        None
    };
    let registry = default_registry(fixture, live)?;

    let run_id = start_discovery_run(
        &pool,
        &DiscoveryRunInsert {
            subject_entity_id: subject_id,
            subject_qid: subject.qid.clone(),
            subject_label: label.into(),
            plan_json: serde_json::to_value(&plan)?,
            budgets_json: serde_json::to_value(&budgets)?,
            connector_versions: serde_json::json!({}),
        },
    )
    .await?;

    let mut metrics = IngestMetrics::default();
    let mut counters = BudgetCounters::default();
    let extractors = default_extractor_stack();
    let extractor_refs: Vec<&dyn CandidateExtractor> =
        extractors.iter().map(|e| e.as_ref()).collect();
    let resolver = GazetteerResolver;
    let projections = DerivedLabelProjections;

    let planned: Vec<_> = plan
        .sources
        .iter()
        .filter(|p| {
            sources_filter
                .as_ref()
                .map(|f| f.iter().any(|s| SourceKind::parse(s) == p.kind))
                .unwrap_or(true)
        })
        .cloned()
        .collect();

    // Prefer fixture when requested.
    let kinds_to_run: Vec<SourceKind> = if use_fixture {
        vec![SourceKind::Fixture]
    } else {
        planned.iter().map(|p| p.kind.clone()).collect()
    };

    for kind in kinds_to_run {
        let Some(reg) = registry.get(&kind) else {
            continue;
        };
        let Some(connector) = &reg.connector else {
            continue;
        };

        let mut cursor = None;
        loop {
            let page = match connector.discover(&subject, cursor.clone()).await {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, source = kind.as_str(), "discover failed");
                    metrics.connector_errors += 1;
                    metrics.bump_loss("connector_discover_failed");
                    break;
                }
            };
            let _ = counters.record_call(&budgets);

            for doc in page.documents {
                metrics.documents_discovered += 1;
                if doc.relevance_score < budgets.min_relevance {
                    let (doc_id, _) =
                        upsert_discovered_document(&pool, &to_discovered_insert(run_id, &doc))
                            .await?;
                    mark_discovered_skipped(&pool, doc_id, "document_irrelevant").await?;
                    metrics.documents_skipped += 1;
                    metrics.bump_loss("document_irrelevant");
                    continue;
                }

                if counters
                    .record_document(kind.as_str(), &budgets, 0)
                    .is_err()
                {
                    metrics.bump_loss("budget_documents");
                    break;
                }

                let (doc_id, _) =
                    upsert_discovered_document(&pool, &to_discovered_insert(run_id, &doc)).await?;

                let fetched = match connector.fetch(&doc).await {
                    Ok(f) => f,
                    Err(e) => {
                        tracing::warn!(error = %e, id = %doc.external_id, "fetch failed");
                        metrics.connector_errors += 1;
                        metrics.bump_loss("connector_fetch_failed");
                        mark_discovered_skipped(&pool, doc_id, "fetch_failed").await?;
                        continue;
                    }
                };
                let _ = counters.record_call(&budgets);
                let _ = counters.record_document(kind.as_str(), &budgets, fetched.content_bytes);

                let hash = content_hash(&fetched.text);
                let source_uri = doc
                    .canonical_url
                    .clone()
                    .unwrap_or_else(|| format!("{}:{}", kind.as_str(), doc.external_id));
                let snapshot_id = insert_document_snapshot(
                    &pool,
                    &DocumentSnapshotInsert {
                        source_type: kind.as_str().into(),
                        source_uri,
                        source_identifier: Some(doc.external_id.clone()),
                        language: doc.language.clone().unwrap_or_else(|| "en".into()),
                        title: Some(doc.title.clone()),
                        content_hash: format!(
                            "{}:{}",
                            hash,
                            fetched.revision_id.clone().unwrap_or_default()
                        ),
                        revision_id: fetched.revision_id.clone(),
                        wiki_page_id: None,
                        raw_document_id: None,
                        text: fetched.text.clone(),
                        metadata: fetched.raw_metadata.clone(),
                    },
                )
                .await?;
                mark_discovered_snapshotted(&pool, doc_id, snapshot_id).await?;
                metrics.documents_snapshotted += 1;

                // Sentence fragment covering whole doc for evidence pointers.
                let frag_id = insert_document_fragment(
                    &pool,
                    &DocumentFragmentInsert {
                        snapshot_id,
                        fragment_kind: "sentence".into(),
                        parent_fragment_id: None,
                        sentence_id: None,
                        text: fetched.text.clone(),
                        start_offset: 0,
                        end_offset: fetched.text.len() as i32,
                        clause_index: None,
                        ordinal: 0,
                    },
                )
                .await?;
                metrics.fragments += 1;

                let input = ExtractorInput {
                    text: fetched.text.clone(),
                    page_title: Some(label.to_string()),
                    subject_label: Some(label.to_string()),
                    document_type: doc.document_type.as_str().to_string(),
                    subject_death_year: subject.death_year,
                };

                let mut raws = Vec::new();
                for ex in &extractor_refs {
                    raws.extend(ex.extract(&input));
                }

                for raw in raws {
                    process_raw_candidate(
                        &pool,
                        config,
                        &subject,
                        subject_id,
                        snapshot_id,
                        frag_id,
                        kind.as_str(),
                        &raw,
                        &resolver,
                        &projections,
                        &mut metrics,
                    )
                    .await?;
                }
            }

            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
    }

    let metrics_json = serde_json::to_value(&metrics_to_json(&metrics))?;
    finish_discovery_run(&pool, run_id, "completed", &metrics_json, None).await?;

    let report = build_ingest_report(&pool, &subject, subject_id, &metrics).await?;
    print!("{report}");
    Ok(report)
}

/// POC-parity harvest: WDQS P710 / P1344 / P607 → quality canonical events.
pub async fn ingest_wdqs_events(
    pool: &sqlx::PgPool,
    config: &AppConfig,
    subject: &ResolvedSubject,
    subject_id: Uuid,
) -> anyhow::Result<IngestMetrics> {
    let qid = subject
        .qid
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("WDQS requires a Wikidata QID"))?;
    let events = if let Ok(dir) = std::env::var("TALARIA_WDQS_FIXTURE") {
        events_from_fixture_dir(std::path::Path::new(&dir))?
    } else {
        tracing::info!(%qid, "fetching WDQS participant/conflict events");
        fetch_events_for_person(qid).await?
    };
    tracing::info!(qid, n = events.len(), "WDQS events after merge");
    let mut metrics = IngestMetrics::default();
    if events.is_empty() {
        return Ok(metrics);
    }
    let text = events_to_statement_text(&events);
    let hash = content_hash(&text);
    let snapshot_id = insert_document_snapshot(
        pool,
        &DocumentSnapshotInsert {
            source_type: "wikidata".into(),
            source_uri: format!("https://query.wikidata.org/sparql#{qid}"),
            source_identifier: Some(format!("wdqs:{qid}")),
            language: "en".into(),
            title: Some(format!("WDQS events for {qid}")),
            content_hash: hash,
            revision_id: Some("wdqs:p710+p1344+p607".into()),
            wiki_page_id: None,
            raw_document_id: None,
            text: text.clone(),
            metadata: serde_json::json!({
                "qid": qid,
                "event_count": events.len(),
                "source": "wdqs"
            }),
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
            text: text.clone(),
            start_offset: 0,
            end_offset: text.len() as i32,
            clause_index: None,
            ordinal: 0,
        },
    )
    .await?;
    metrics.documents_snapshotted += 1;
    metrics.fragments += 1;

    let input = ExtractorInput {
        text,
        page_title: Some(subject.label.clone()),
        subject_label: Some(subject.label.clone()),
        document_type: "structured_statement".into(),
        subject_death_year: subject.death_year,
    };
    let raws = StructuredStatementExtractor.extract(&input);
    let resolver = GazetteerResolver;
    let projections = DerivedLabelProjections;
    for raw in raws {
        process_raw_candidate(
            pool,
            config,
            subject,
            subject_id,
            snapshot_id,
            frag_id,
            "wikidata",
            &raw,
            &resolver,
            &projections,
            &mut metrics,
        )
        .await?;
    }
    Ok(metrics)
}

fn to_discovered_insert(run_id: Uuid, doc: &DiscoveredDocument) -> DiscoveredDocumentInsert {
    DiscoveredDocumentInsert {
        run_id,
        source_kind: doc.source_kind.as_str().into(),
        external_id: doc.external_id.clone(),
        canonical_url: doc.canonical_url.clone(),
        title: doc.title.clone(),
        language: doc.language.clone(),
        document_type: doc.document_type.as_str().into(),
        discovery_method: doc.discovery_method.as_str().into(),
        relevance_score: doc.relevance_score,
        subject_links: serde_json::to_value(&doc.subject_links).unwrap_or_default(),
        source_metadata: doc.source_metadata.raw.clone(),
    }
}

fn metrics_to_json(m: &IngestMetrics) -> serde_json::Value {
    serde_json::json!({
        "documents_discovered": m.documents_discovered,
        "documents_snapshotted": m.documents_snapshotted,
        "documents_skipped": m.documents_skipped,
        "fragments": m.fragments,
        "candidates": m.candidates,
        "candidates_deduped": m.candidates_deduped,
        "accepted": m.accepted,
        "rejected": m.rejected,
        "needs_review": m.needs_review,
        "claims_created": m.claims_created,
        "claims_reinforced": m.claims_reinforced,
        "events_created": m.events_created,
        "events_reinforced": m.events_reinforced,
        "connector_errors": m.connector_errors,
        "loss_reasons": m.loss_reasons,
    })
}

#[allow(clippy::too_many_arguments)]
async fn process_raw_candidate(
    pool: &sqlx::PgPool,
    config: &AppConfig,
    subject: &ResolvedSubject,
    subject_id: Uuid,
    snapshot_id: Uuid,
    frag_id: Uuid,
    source_kind: &str,
    raw: &talaria_sources::extractors::RawCandidate,
    resolver: &GazetteerResolver,
    projections: &DerivedLabelProjections,
    metrics: &mut IngestMetrics,
) -> anyhow::Result<()> {
    let time = parse_typed_time(raw.time_surface.as_deref());
    let mut shell = talaria_quality::EventCandidate {
        id: Uuid::nil(),
        snapshot_id,
        fragment_id: frag_id,
        clause_index: raw.clause_index,
        subject_surface: raw.subject_surface.clone(),
        subject_entity_id: Some(subject_id),
        event_type: raw.event_type.clone(),
        predicate: raw.predicate.clone(),
        time: time.clone(),
        place_mentions: vec![],
        object_mentions: vec![],
        participant_mentions: vec![],
        place_entity_id: None,
        place_label: None,
        evidence_ptrs: vec![],
        extractor_version: raw.extractor_id.clone(),
        fingerprint: String::new(),
        status: talaria_quality::CandidateStatus::Pending,
        rejection_codes: vec![],
    };

    let resolved = resolve_mentions(
        &shell,
        resolver,
        raw.place_surface.as_deref(),
        raw.object_surface.as_deref(),
        &[],
    );
    shell.place_mentions = resolved.place_mentions;
    shell.object_mentions = resolved.object_mentions;
    shell.participant_mentions = resolved.participant_mentions.clone();
    shell.place_label = resolved.place_label.clone();
    if raw.extractor_id == "structured_statement" && !resolved.invalid_place_attempt {
        if let Some(label) = shell
            .place_label
            .clone()
            .or_else(|| raw.place_surface.clone())
            .filter(|s| !s.is_empty())
        {
            shell.place_label = Some(label.clone());
            shell.place_mentions = vec![Mention {
                surface: label.clone(),
                entity_id: None,
                kind: Some(EntityKind::Place),
                role: None,
            }];
            shell.place_entity_id =
                Some(upsert_entity_with_kind(pool, &config.wiki_lang, &label, "place").await?);
        }
    } else if resolved.place_kind == Some(EntityKind::Place) {
        if let Some(label) = &shell.place_label {
            shell.place_entity_id =
                Some(upsert_entity_with_kind(pool, &config.wiki_lang, label, "place").await?);
        }
    }
    for m in &mut shell.participant_mentions {
        let kind = m.kind.unwrap_or(EntityKind::Unknown).as_str();
        m.entity_id =
            Some(upsert_entity_with_kind(pool, &config.wiki_lang, &m.surface, kind).await?);
    }

    shell.evidence_ptrs = vec![EvidencePtr {
        fragment_id: frag_id,
        clause_index: raw.clause_index,
        start_offset: raw.start_offset,
        end_offset: raw.end_offset,
        quoted_text: raw.clause_text.clone(),
    }];

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

    let primary_object = shell
        .object_mentions
        .first()
        .map(|m| m.surface.clone())
        .or_else(|| raw.object_surface.clone());
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
            object_mentions: serde_json::to_value(&shell.object_mentions)?,
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
            judgment_json: serde_json::json!({}),
        },
    )
    .await?;
    shell.id = cand_id;
    let is_new_candidate = inserted;
    let mut skip_gates = false;
    if inserted {
        metrics.candidates += 1;
    } else {
        let existing = get_event_candidate_by_fingerprint(pool, &shell.fingerprint)
            .await?
            .ok_or_else(|| anyhow::anyhow!("fingerprint conflict without existing row"))?;
        match existing_candidate_action(&existing.status) {
            ExistingCandidateAction::SkipTerminal => {
                metrics.candidates_deduped += 1;
                return Ok(());
            }
            ExistingCandidateAction::ResumeAssembleOnly => {
                skip_gates = true;
            }
            ExistingCandidateAction::ResumeFromGates => {}
        }
    }

    if !skip_gates {
        let (birth_year, death_year, has_birth, has_death) =
            quality_lifespan_years(pool, subject_id).await?;
        let place_entity_kind = match shell.place_entity_id {
            Some(pid) => talaria_store::get_entity_kind(pool, pid)
                .await?
                .map(|s| EntityKind::parse(&s)),
            None => None,
        };
        let ctx = GateContext {
            subject_birth_year: birth_year.or(subject.birth_year),
            subject_death_year: death_year.or(subject.death_year),
            has_active_birth: has_birth,
            has_active_death: has_death,
            fingerprint_exists: false,
            cross_clause_join_detected: raw.cross_clause_join,
            place_entity_kind,
        };
        let decision = apply_gates(&shell, &ctx);
        let status = decision.status().as_str();
        let codes = decision.codes();
        for c in &codes {
            metrics.bump_loss(c);
        }
        update_event_candidate_judgment(
            pool,
            cand_id,
            status,
            &codes,
            &serde_json::json!({"codes": codes, "source": source_kind}),
            shell.subject_entity_id,
            shell.place_entity_id,
            shell.place_label.as_deref(),
            &serde_json::to_value(&shell.place_mentions)?,
            &serde_json::to_value(&shell.object_mentions)?,
            &serde_json::to_value(&shell.participant_mentions)?,
        )
        .await?;

        match status {
            "accepted" => metrics.accepted += 1,
            "rejected" => {
                metrics.rejected += 1;
                return Ok(());
            }
            "needs_review" => {
                metrics.needs_review += 1;
                return Ok(());
            }
            _ => {}
        }
    }

    // Claim consolidation
    let claim_fp = claim_fingerprint(&ClaimKey {
        subject: subject.label.clone(),
        predicate: shell.predicate.clone(),
        object_or_value: shell
            .participant_mentions
            .first()
            .map(|m| m.surface.clone())
            .unwrap_or_default(),
        time_key: shell.time.canonical_key(),
        place_key: shell.place_label.clone().unwrap_or_default(),
    });
    let (claim_id, claim_new) = upsert_quality_claim(
        pool,
        &QualityClaimInsert {
            subject_entity_id: subject_id,
            fingerprint: claim_fp,
            predicate: shell.predicate.clone(),
            event_type: shell.event_type.clone(),
            object_json: serde_json::json!({}),
            time_json: time_to_json(&shell.time),
            place_entity_id: shell.place_entity_id,
            place_label: shell.place_label.clone(),
            occurrence_stem: Some(stem.clone()),
        },
    )
    .await?;
    if claim_new {
        metrics.claims_created += 1;
    } else {
        metrics.claims_reinforced += 1;
    }
    add_claim_support(
        pool,
        claim_id,
        Some(cand_id),
        Some(snapshot_id),
        source_kind,
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
            &serde_json::json!({"abstain": true, "places": places, "source": source_kind}),
            shell.subject_entity_id,
            shell.place_entity_id,
            shell.place_label.as_deref(),
            &serde_json::to_value(&shell.place_mentions)?,
            &serde_json::to_value(&shell.object_mentions)?,
            &serde_json::to_value(&shell.participant_mentions)?,
        )
        .await?;
        metrics.needs_review += 1;
        metrics.accepted = metrics.accepted.saturating_sub(1);
        metrics.bump_loss("competing_place");
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
                &serde_json::to_value(&shell.place_mentions)?,
                &serde_json::to_value(&shell.object_mentions)?,
                &serde_json::to_value(&shell.participant_mentions)?,
            )
            .await?;
            metrics.rejected += 1;
            metrics.accepted = metrics.accepted.saturating_sub(1);
            metrics.bump_loss("singleton_cardinality_violation");
            return Ok(());
        }
    }

    let proj = projections.from_candidate(&shell, &subject.label);
    let title_derived = projections.display_label(&proj);
    let (lat, lon, map_eligible) = if raw.lat.is_some() && raw.lon.is_some() {
        (raw.lat, raw.lon, true)
    } else {
        let place = shell.place_label.as_deref().map(parse_place_surface);
        let map_eligible = place.as_ref().is_some_and(|p| p.map_eligible());
        let (lat, lon) = place
            .as_ref()
            .map(|p| (p.lat, p.lon))
            .unwrap_or((None, None));
        (lat, lon, map_eligible)
    };

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
            confidence: 0.8,
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
    mark_candidate_assembled(pool, cand_id, event_id).await?;
    link_claim_to_event(pool, claim_id, event_id).await?;
    metrics.events_created += 1;
    Ok(())
}

async fn build_ingest_report(
    pool: &sqlx::PgPool,
    subject: &ResolvedSubject,
    subject_id: Uuid,
    metrics: &IngestMetrics,
) -> anyhow::Result<String> {
    let density = density_report_counts(pool, Some(subject_id)).await?;
    let legacy_facts: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint FROM canonical_events ce
        JOIN entities e ON e.id = ce.entity_id
        WHERE ce.pipeline = 'legacy'
          AND (e.wikipedia_title ILIKE $1 OR e.canonical_name ILIKE $1 OR ce.title ILIKE $1)
        "#,
    )
    .bind(format!("%{}%", subject.label))
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    let legacy_map: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint FROM canonical_events ce
        JOIN entities e ON e.id = ce.entity_id
        WHERE ce.pipeline = 'legacy' AND ce.map_eligible
          AND (e.wikipedia_title ILIKE $1 OR e.canonical_name ILIKE $1 OR ce.title ILIKE $1)
        "#,
    )
    .bind(format!("%{}%", subject.label))
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let report = serde_json::json!({
        "subject": {
            "entity_id": subject_id,
            "qid": subject.qid,
            "label": subject.label,
        },
        "run_metrics": metrics_to_json(metrics),
        "global": {
            "documents_discovered": density.documents_discovered,
            "documents_snapshotted": density.documents_snapshotted,
            "fragments": density.fragments,
            "candidates": density.candidates,
            "rejected": density.rejected,
            "needs_review": density.needs_review,
            "claims": density.claims,
            "accepted_events": density.accepted_events,
            "timeline_eligible": density.timeline_eligible,
            "map_eligible": density.map_eligible,
            "events_without_place": density.events_without_place,
            "events_reinforced_by_multiple_sources": density.multi_source_events,
        },
        "comparison": {
            "legacy_events": legacy_facts,
            "legacy_map_eligible": legacy_map,
            "quality_active_events": density.accepted_events,
            "quality_map_eligible": density.map_eligible,
            "quality_timeline_eligible": density.timeline_eligible,
        }
    });
    Ok(serde_json::to_string_pretty(&report)?)
}
