// crates/talaria-api/src/corpus_ingest.rs
//! Corpus ingest orchestration (PR1): discover → fetch → snapshot → normalize → persist.
//! Does NOT create quality claims, soft claims, events, or historiography positions.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::task::JoinSet;

use talaria_core::AppConfig;
use talaria_sources::connectors::{
    default_registry_with_corpus, normalize_hal_doc, BnfConfig, BnfConnector, CorpusConnectors,
    EuropeanaConfig, EuropeanaConnector, HalConnector, InternetArchiveConfig,
    InternetArchiveConnector, OpenAlexConfig, OpenAlexConnector, PerseeConnector, ThesesFrConfig,
    ThesesFrConnector, WikisourceConnector,
};
use talaria_sources::{
    match_resolved_subject_to_document, normalize_bnf_notice, normalize_europeana_item,
    normalize_ia_item, normalize_openalex_work, normalize_these_detail, normalize_wikisource,
    AccessLevel, DiscoveredDocument, DiscoveryCursor, FetchedDocument,
    NormalizedCorpusDocument, ResolvedSubject, SourceConnector, SourceKind, TypedTimeLite,
};
use talaria_store::{
    connect, finish_discovery_run, insert_document_snapshot, link_corpus_snapshot,
    mark_discovered_corpus_document, mark_discovered_skipped, mark_discovered_snapshotted,
    replace_document_contributions, replace_document_identifiers, replace_document_subjects,
    run_migrations, start_discovery_run, upsert_corpus_document, upsert_discovered_document,
    upsert_entity_document_link, upsert_entity_with_kind, CorpusDocumentInsert,
    DiscoveredDocumentInsert, DiscoveryRunInsert, DocumentSnapshotInsert,
};
use uuid::Uuid;

fn typed_time_json(t: &TypedTimeLite) -> serde_json::Value {
    serde_json::to_value(t).unwrap_or_else(|_| serde_json::json!({"kind":"unknown"}))
}

/// Catalogs linked from explorer search / live ingest (not Wikipedia identity).
pub const LIVE_CORPUS_PROVIDERS: &[&str] = &[
    "hal",
    "persee",
    "theses_fr",
    "gallica",
    "open_library",
    "open_alex",
    "internet_archive",
    "europeana",
    "bnf",
];

/// Sister Wikimedia projects that yield dated captions / transcriptions.
pub const LIVE_WIKI_SISTER_PROVIDERS: &[&str] = &["wikisource", "wikimedia_commons"];

pub fn live_corpus_providers() -> Vec<String> {
    LIVE_CORPUS_PROVIDERS
        .iter()
        .map(|name| (*name).to_string())
        .collect()
}

/// Explorer quality ingest: Wikisource + Commons life-trace only.
/// Bibliographic catalogs stay on the Agora lane.
pub fn explorer_fact_providers() -> Vec<String> {
    LIVE_WIKI_SISTER_PROVIDERS
        .iter()
        .map(|name| (*name).to_string())
        .collect()
}

/// Empty live list → every catalog. Empty fixture list stays theses.fr (CLI default).
pub fn resolve_corpus_providers(providers: &[String], live: bool) -> Vec<SourceKind> {
    let names: Vec<String> = if providers.is_empty() {
        if live {
            live_corpus_providers()
        } else {
            vec!["theses_fr".into()]
        }
    } else {
        providers.to_vec()
    };
    names.iter().map(|name| SourceKind::parse(name)).collect()
}

fn wants(kinds: &[SourceKind], kind: SourceKind) -> bool {
    kinds.iter().any(|candidate| candidate == &kind)
}

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct CorpusIngestMetrics {
    pub documents_discovered: u64,
    pub documents_persisted: u64,
    pub documents_skipped: u64,
    pub snapshots_created: u64,
    pub snapshots_reused: u64,
    pub entity_links: u64,
    pub connector_errors: u64,
    pub fetch_timeouts: u64,
    pub providers: BTreeMap<String, ProviderIngestMetrics>,
}

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct ProviderIngestMetrics {
    pub status: String,
    pub connector_version: Option<String>,
    pub discover_calls: u64,
    pub documents_discovered: u64,
    pub documents_persisted: u64,
    pub documents_skipped: u64,
    pub connector_errors: u64,
    pub fetch_calls: u64,
    pub fetch_timeouts: u64,
    pub elapsed_ms: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct CorpusIngestLimits {
    pub per_provider: u32,
    pub total: Option<u32>,
    pub minimum_per_provider: u32,
    pub provider_timeout: Duration,
}

impl CorpusIngestLimits {
    pub fn legacy(per_provider: u32) -> Self {
        Self {
            per_provider,
            total: None,
            minimum_per_provider: 0,
            provider_timeout: Duration::from_secs(30),
        }
    }
}

fn provider_budget(
    limits: CorpusIngestLimits,
    provider_count: usize,
    provider_index: usize,
    globally_persisted: usize,
) -> u32 {
    let Some(total) = limits.total else {
        return limits.per_provider;
    };
    let fair_minimum = limits
        .minimum_per_provider
        .min(limits.per_provider)
        .min(total / provider_count.max(1) as u32);
    let providers_after = provider_count.saturating_sub(provider_index + 1) as u32;
    let globally_remaining = total.saturating_sub(globally_persisted as u32);
    globally_remaining
        .saturating_sub(fair_minimum.saturating_mul(providers_after))
        .min(limits.per_provider)
}

fn connector_failure_status(error: &str) -> &'static str {
    if error.contains("429") || error.to_ascii_lowercase().contains("rate limit") {
        "rate_limited"
    } else {
        "failed"
    }
}

// Fetch only metadata in bounded groups. Database writes remain sequential and
// source-ordered, keeping unique-document caps and attribution deterministic.
const DEFAULT_FETCH_CONCURRENCY: usize = 2;
const MAX_FETCH_CONCURRENCY: usize = 4;

fn resolved_fetch_concurrency(configured: Option<&str>) -> usize {
    configured
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_FETCH_CONCURRENCY)
        .clamp(1, MAX_FETCH_CONCURRENCY)
}

fn cursor_stalled(previous: &Option<DiscoveryCursor>, next: &Option<DiscoveryCursor>) -> bool {
    match (previous, next) {
        (Some(previous), Some(next)) => {
            previous.token == next.token && previous.offset == next.offset
        }
        _ => false,
    }
}

type FetchResult = (Uuid, DiscoveredDocument, Result<FetchedDocument, String>);

async fn fetch_document_batch(
    connector: Arc<dyn SourceConnector>,
    documents: Vec<(Uuid, DiscoveredDocument)>,
    timeout: Duration,
) -> anyhow::Result<Vec<FetchResult>> {
    let mut tasks = JoinSet::new();
    for (index, (discovered_id, document)) in documents.into_iter().enumerate() {
        let connector = Arc::clone(&connector);
        tasks.spawn(async move {
            let fetched = match tokio::time::timeout(timeout, connector.fetch(&document)).await {
                Ok(Ok(fetched)) => Ok(fetched),
                Ok(Err(error)) => Err(error.to_string()),
                Err(_) => Err(format!("fetch timed out after {}ms", timeout.as_millis())),
            };
            (index, discovered_id, document, fetched)
        });
    }
    let mut fetched = Vec::with_capacity(tasks.len());
    while let Some(result) = tasks.join_next().await {
        fetched.push(result.map_err(|error| anyhow::anyhow!("fetch worker failed: {error}"))?);
    }
    fetched.sort_by_key(|(index, _, _, _)| *index);
    Ok(fetched.into_iter()
        .map(|(_, id, document, fetched)| (id, document, fetched))
        .collect())
}

pub async fn run_corpus_ingest(
    config: &AppConfig,
    subject_label: &str,
    qid: Option<&str>,
    providers: &[String],
    limits: CorpusIngestLimits,
    use_fixture: bool,
    fixture_dir: Option<PathBuf>,
    live: bool,
) -> anyhow::Result<String> {
    let use_fixture = use_fixture && !live;
    let pool = connect(config).await?;
    run_migrations(&pool).await?;

    let subject_id = if let Some(qid) = qid.map(str::trim).filter(|value| !value.is_empty()) {
        talaria_store::upsert_person_by_qid(
            &pool,
            qid,
            subject_label,
            &config.wiki_lang,
            subject_label,
            subject_label,
        )
        .await?
    } else {
        upsert_entity_with_kind(&pool, &config.wiki_lang, subject_label, "person").await?
    };
    let mut subject = ResolvedSubject {
        entity_id: Some(subject_id),
        qid: qid.map(str::to_string),
        label: subject_label.into(),
        languages: vec!["fr".into(), "en".into()],
        birth_year: None,
        death_year: None,
        countries: vec![],
        occupations: vec![],
        known_identifiers: qid
            .map(|q| vec![("wikidata".into(), q.to_string())])
            .unwrap_or_default(),
    };
    if live {
        if let Some(q) = subject.qid.clone() {
            if let Ok(meta) =
                crate::lot_e::fetch_wikidata_subject_meta(&q, &config.wiki_lang, Some(&pool)).await
            {
                crate::lot_e::append_wikidata_meta_identifiers(
                    &mut subject.known_identifiers,
                    &meta,
                );
                if !meta.occupations.is_empty() {
                    subject.occupations = meta.occupations;
                }
                subject.birth_year = meta.birth_year;
                subject.death_year = meta.death_year;
            }
        }
    }

    let kinds = resolve_corpus_providers(providers, live);
    let want_theses = wants(&kinds, SourceKind::ThesesFr);
    let want_openalex = wants(&kinds, SourceKind::OpenAlex);
    let want_ia = wants(&kinds, SourceKind::InternetArchive);
    let want_europeana = wants(&kinds, SourceKind::Europeana);
    let want_bnf = wants(&kinds, SourceKind::Bnf);
    let want_hal = wants(&kinds, SourceKind::Hal);
    let want_persee = wants(&kinds, SourceKind::Persee);

    let theses = if want_theses {
        if use_fixture {
            let dir = fixture_dir
                .clone()
                .unwrap_or_else(|| PathBuf::from("fixtures/theses_fr"));
            Some(ThesesFrConnector::from_fixture_dir(&dir)?)
        } else if live {
            Some(ThesesFrConnector::new(ThesesFrConfig::default())?)
        } else {
            anyhow::bail!("corpus-ingest theses_fr requires --fixture or --live");
        }
    } else {
        None
    };

    let open_alex = if want_openalex {
        if use_fixture {
            let dir = PathBuf::from("fixtures/open_alex");
            Some(OpenAlexConnector::from_fixture_dir(&dir)?)
        } else if live {
            let mut cfg = OpenAlexConfig::default();
            cfg.api_key = std::env::var("OPENALEX_API_KEY")
                .ok()
                .filter(|s| !s.trim().is_empty());
            cfg.mailto = std::env::var("OPENALEX_MAILTO")
                .ok()
                .filter(|s| !s.trim().is_empty());
            Some(OpenAlexConnector::new(cfg)?)
        } else {
            anyhow::bail!("corpus-ingest open_alex requires --fixture or --live");
        }
    } else {
        None
    };

    let internet_archive = if want_ia {
        if use_fixture {
            Some(InternetArchiveConnector::from_fixture_dir(
                "fixtures/internet_archive",
            )?)
        } else if live {
            Some(InternetArchiveConnector::new(
                InternetArchiveConfig::default(),
            )?)
        } else {
            anyhow::bail!("corpus-ingest internet_archive requires --fixture or --live");
        }
    } else {
        None
    };

    let europeana = if want_europeana {
        if use_fixture {
            Some(EuropeanaConnector::from_fixture_dir("fixtures/europeana")?)
        } else if live {
            let api_key = std::env::var("EUROPEANA_API_KEY")
                .ok()
                .filter(|s| !s.trim().is_empty());
            Some(EuropeanaConnector::new(EuropeanaConfig {
                api_key,
                ..EuropeanaConfig::default()
            })?)
        } else {
            anyhow::bail!("corpus-ingest europeana requires --fixture or --live");
        }
    } else {
        None
    };

    let bnf = if want_bnf {
        if use_fixture {
            Some(BnfConnector::from_fixture_dir("fixtures/bnf")?)
        } else if live {
            Some(BnfConnector::new(BnfConfig::default())?)
        } else {
            anyhow::bail!("corpus-ingest bnf requires --fixture or --live");
        }
    } else {
        None
    };

    let hal = if want_hal {
        if live {
            Some(HalConnector::new()?)
        } else {
            anyhow::bail!("corpus-ingest hal requires --live");
        }
    } else {
        None
    };

    let persee = if want_persee {
        if live {
            Some(PerseeConnector::new()?)
        } else {
            anyhow::bail!("corpus-ingest persee requires --live");
        }
    } else {
        None
    };

    let registry = default_registry_with_corpus(
        None,
        live,
        CorpusConnectors {
            theses_fr: theses,
            open_alex,
            internet_archive,
            europeana,
            bnf,
            hal,
            persee,
        },
    )?;

    let run_id = start_discovery_run(
        &pool,
        &DiscoveryRunInsert {
            subject_entity_id: subject_id,
            subject_qid: subject.qid.clone(),
            subject_label: subject_label.into(),
            plan_json: serde_json::json!({
                "mode": "corpus_ingest",
                "providers": kinds.iter().map(|k| k.as_str()).collect::<Vec<_>>(),
                "limit_per_provider": limits.per_provider,
                "limit_total": limits.total,
                "minimum_per_provider": limits.minimum_per_provider,
                "fetch_concurrency": resolved_fetch_concurrency(
                    std::env::var("TALARIA_CORPUS_FETCH_CONCURRENCY").ok().as_deref()
                ),
                "fetch_timeout_ms": limits.provider_timeout.as_millis(),
                "fixture": use_fixture,
            }),
            budgets_json: serde_json::json!({
                "limit_per_provider": limits.per_provider,
                "limit_total": limits.total,
                "minimum_per_provider": limits.minimum_per_provider,
            }),
            connector_versions: serde_json::Value::Object(
                kinds
                    .iter()
                    .filter_map(|kind| {
                        registry
                            .get(kind)
                            .and_then(|registration| registration.connector.as_ref())
                            .map(|connector| {
                                (
                                    kind.as_str().to_string(),
                                    serde_json::json!(connector.connector_version()),
                                )
                            })
                    })
                    .collect(),
            ),
        },
    )
    .await?;

    let mut metrics = CorpusIngestMetrics::default();
    let mut persisted_ids = HashSet::new();

    let provider_count = kinds.len();
    let fetch_concurrency = resolved_fetch_concurrency(
        std::env::var("TALARIA_CORPUS_FETCH_CONCURRENCY").ok().as_deref(),
    );
    for (provider_index, kind) in kinds.into_iter().enumerate() {
        let started = Instant::now();
        let source = kind.as_str().to_string();
        let mut provider_metrics = ProviderIngestMetrics {
            status: "not_configured".into(),
            ..ProviderIngestMetrics::default()
        };
        let Some(reg) = registry.get(&kind) else {
            metrics.providers.insert(source, provider_metrics);
            continue;
        };
        let Some(connector) = &reg.connector else {
            metrics.providers.insert(source, provider_metrics);
            continue;
        };
        provider_metrics.connector_version = Some(connector.connector_version().to_string());
        if !reg.implemented {
            tracing::warn!(source = kind.as_str(), "connector not implemented; skip");
            metrics.providers.insert(source, provider_metrics);
            continue;
        }

        let mut cursor = None;
        // Let earlier providers use surplus capacity, but reserve the requested first
        // sample for every provider that has not run yet. If the total is smaller than
        // the requested aggregate minimum, divide it as evenly as possible.
        let mut provider_remaining =
            provider_budget(limits, provider_count, provider_index, persisted_ids.len());
        provider_metrics.status = "healthy".into();
        loop {
            // Make one discovery call even when the persistence budget is exhausted,
            // so every configured provider reports its real health for this run.
            if provider_remaining == 0 && provider_metrics.discover_calls > 0 {
                break;
            }
            provider_metrics.discover_calls += 1;
            let page = match tokio::time::timeout(
                limits.provider_timeout,
                connector.discover(&subject, cursor.clone()),
            )
            .await
            {
                Ok(Ok(p)) => p,
                Err(_) => {
                    let error = format!(
                        "provider timed out after {}s",
                        limits.provider_timeout.as_secs()
                    );
                    tracing::warn!(source = kind.as_str(), %error, "discover failed");
                    provider_metrics.status = "failed".into();
                    provider_metrics.connector_errors += 1;
                    provider_metrics.last_error = Some(error);
                    metrics.connector_errors += 1;
                    break;
                }
                Ok(Err(e)) => {
                    tracing::warn!(error = %e, source = kind.as_str(), "discover failed");
                    provider_metrics.status = connector_failure_status(&e.to_string()).into();
                    provider_metrics.connector_errors += 1;
                    provider_metrics.last_error = Some(e.to_string());
                    metrics.connector_errors += 1;
                    break;
                }
            };

            let mut documents = page.documents.into_iter();
            let mut stop_provider = false;
            loop {
                // Do not fetch beyond this provider's budget or the global unique cap.
                // When a fetch fails we can try subsequent discovered documents.
                let global_remaining = limits
                    .total
                    .map(|total| total.saturating_sub(persisted_ids.len() as u32))
                    .unwrap_or(u32::MAX);
                let batch_size = (provider_remaining.min(global_remaining) as usize)
                    .min(fetch_concurrency);
                if batch_size == 0 {
                    break;
                }
                let mut batch = Vec::with_capacity(batch_size);
                for doc in documents.by_ref().take(batch_size) {
                    metrics.documents_discovered += 1;
                    provider_metrics.documents_discovered += 1;
                    let (discovered_id, _) =
                        upsert_discovered_document(&pool, &to_discovered_insert(run_id, &doc)).await?;
                    batch.push((discovered_id, doc));
                }
                if batch.is_empty() {
                    break;
                }

                // Connector requests are concurrent only within the bounded batch;
                // their database writes are applied in original discovery order.
                let fetched_batch = fetch_document_batch(
                    Arc::clone(connector),
                    batch,
                    limits.provider_timeout,
                ).await?;
                for (discovered_id, doc, fetched) in fetched_batch {
                    provider_metrics.fetch_calls += 1;
                    let fetched = match fetched {
                        Ok(fetched) => fetched,
                        Err(error) => {
                            tracing::warn!(
                                source = kind.as_str(),
                                id = %doc.external_id,
                                %error,
                                "corpus fetch failed"
                            );
                            metrics.connector_errors += 1;
                            provider_metrics.connector_errors += 1;
                            provider_metrics.last_error = Some(error.clone());
                            let timed_out = error.starts_with("fetch timed out");
                            if timed_out {
                                metrics.fetch_timeouts += 1;
                                provider_metrics.fetch_timeouts += 1;
                            }
                            let rate_limited = connector_failure_status(&error) == "rate_limited";
                            provider_metrics.status = if rate_limited {
                                "rate_limited".into()
                            } else {
                                "partial".into()
                            };
                            mark_discovered_skipped(
                                &pool,
                                discovered_id,
                                if timed_out { "fetch_timeout" } else { "fetch_failed" },
                            ).await?;
                            metrics.documents_skipped += 1;
                            provider_metrics.documents_skipped += 1;
                            if rate_limited {
                                stop_provider = true;
                            }
                            continue;
                        }
                    };
                let Some(normalized) = extract_normalized(&kind, &fetched.raw_metadata)? else {
                    tracing::warn!(
                        source = kind.as_str(),
                        id = %doc.external_id,
                        "no corpus normalizer; skip bibliography persist"
                    );
                    mark_discovered_skipped(&pool, discovered_id, "no_normalizer").await?;
                    metrics.documents_skipped += 1;
                    provider_metrics.documents_skipped += 1;
                    continue;
                };
                let (corpus_id, snapshot_id, snapshot_new) =
                    persist_normalized(&pool, &kind, &doc, &normalized, None).await?;

                if kind == SourceKind::Wikisource && !normalized.snapshot_text.trim().is_empty() {
                    crate::wiki_persist::persist_wiki_fragments(
                        &pool,
                        snapshot_id,
                        &normalized.snapshot_text,
                    )
                    .await?;
                }

                mark_discovered_corpus_document(&pool, discovered_id, corpus_id).await?;
                mark_discovered_snapshotted(&pool, discovered_id, snapshot_id).await?;
                metrics.documents_persisted += 1;
                provider_metrics.documents_persisted += 1;
                persisted_ids.insert(corpus_id);
                if snapshot_new {
                    metrics.snapshots_created += 1;
                } else {
                    metrics.snapshots_reused += 1;
                }

                if let Some(m) = match_resolved_subject_to_document(&subject, &normalized) {
                    let components = serde_json::to_value(&m.components)?;
                    upsert_entity_document_link(
                        &pool,
                        &talaria_store::EntityDocumentLinkInsert {
                            entity_id: subject_id,
                            corpus_document_id: corpus_id,
                            relation: m.relation.clone(),
                            match_version: m.match_version.clone(),
                            score: m.score,
                            components,
                            evidence_summary: Some(m.evidence_summary.clone()),
                        },
                    )
                    .await?;
                    metrics.entity_links += 1;
                }

                provider_remaining = provider_remaining.saturating_sub(1);
                }
                // Never continue hammering a provider after HTTP 429.
                if stop_provider {
                    break;
                }
            }

            let next_cursor = page.next_cursor;
            if cursor_stalled(&cursor, &next_cursor) {
                provider_metrics.status = "partial".into();
                provider_metrics.connector_errors += 1;
                provider_metrics.last_error = Some("discovery cursor did not advance".into());
                metrics.connector_errors += 1;
                break;
            }
            cursor = next_cursor;
            if cursor.is_none() || stop_provider {
                break;
            }
        }
        provider_metrics.elapsed_ms =
            started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        metrics.providers.insert(source, provider_metrics);
    }

    let metrics_json = serde_json::to_value(&metrics)?;
    finish_discovery_run(&pool, run_id, "completed", &metrics_json, None).await?;

    let report = serde_json::json!({
        "run_id": run_id,
        "subject_entity_id": subject_id,
        "subject": subject_label,
        "metrics": metrics,
        "note": "corpus ingest does not create claims or events",
    });
    let pretty = serde_json::to_string_pretty(&report)?;
    println!("{pretty}");
    Ok(pretty)
}

pub(crate) fn extract_normalized(
    kind: &SourceKind,
    raw_metadata: &serde_json::Value,
) -> anyhow::Result<Option<NormalizedCorpusDocument>> {
    if let Some(n) = raw_metadata.get("normalized") {
        return Ok(Some(serde_json::from_value(n.clone())?));
    }
    match kind {
        SourceKind::ThesesFr => {
            let provider = raw_metadata
                .get("provider")
                .cloned()
                .unwrap_or_else(|| raw_metadata.clone());
            Ok(Some(normalize_these_detail(&provider)?))
        }
        SourceKind::OpenAlex => {
            let provider = raw_metadata
                .get("provider")
                .cloned()
                .unwrap_or_else(|| raw_metadata.clone());
            Ok(Some(normalize_openalex_work(&provider)?))
        }
        SourceKind::InternetArchive => {
            let provider = raw_metadata
                .get("provider")
                .cloned()
                .unwrap_or_else(|| raw_metadata.clone());
            Ok(Some(normalize_ia_item(&provider)?))
        }
        SourceKind::Europeana => {
            let provider = raw_metadata
                .get("provider")
                .cloned()
                .unwrap_or_else(|| raw_metadata.clone());
            Ok(Some(normalize_europeana_item(&provider)?))
        }
        SourceKind::Bnf => {
            let provider = raw_metadata
                .get("provider")
                .cloned()
                .unwrap_or_else(|| raw_metadata.clone());
            Ok(Some(normalize_bnf_notice(&provider)?))
        }
        SourceKind::Hal => {
            let provider = raw_metadata
                .get("provider")
                .cloned()
                .unwrap_or_else(|| raw_metadata.clone());
            Ok(Some(normalize_hal_doc(&provider)?))
        }
        SourceKind::Wikisource => {
            let title = raw_metadata
                .get("title")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("");
            if title.is_empty() {
                return Ok(None);
            }
            let wikitext = raw_metadata
                .get("wikitext")
                .or_else(|| raw_metadata.get("snapshot_text"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let doc = WikisourceConnector::document_from_title(title);
            Ok(Some(normalize_wikisource(&doc, wikitext, raw_metadata)?))
        }
        _ => Ok(None),
    }
}

pub(crate) async fn persist_normalized(
    pool: &sqlx::PgPool,
    kind: &SourceKind,
    discovered: &DiscoveredDocument,
    n: &NormalizedCorpusDocument,
    existing_snapshot: Option<Uuid>,
) -> anyhow::Result<(Uuid, Uuid, bool)> {
    let corpus_id = upsert_corpus_from_normalized(pool, n).await?;

    let hash = n.content_fingerprint();
    let source_uri = n
        .canonical_url
        .clone()
        .unwrap_or_else(|| format!("{}:{}", kind.as_str(), n.external_id));
    let content_hash_key = hash.clone();

    let (snapshot_id, snapshot_new) = if let Some(snapshot_id) = existing_snapshot {
        (snapshot_id, false)
    } else {
        let before = talaria_store::count_corpus_snapshots(pool, corpus_id).await?;
        let snapshot_id = insert_document_snapshot(
            pool,
            &DocumentSnapshotInsert {
                source_type: kind.as_str().into(),
                source_uri,
                source_identifier: Some(n.external_id.clone()),
                language: n.language.clone().unwrap_or_else(|| "fr".into()),
                title: Some(n.title.clone()),
                content_hash: content_hash_key.clone(),
                revision_id: n.revision_token.clone(),
                wiki_page_id: None,
                raw_document_id: None,
                text: if n.rights_normalized == AccessLevel::Open
                    || n.access_level == AccessLevel::MetadataOnly
                {
                    n.snapshot_text.clone()
                } else {
                    String::new()
                },
                metadata: serde_json::json!({
                    "corpus_document_id": corpus_id,
                    "discovered_external_id": discovered.external_id,
                    "discovered_source_kind": discovered.source_kind.as_str(),
                    "discovered_canonical_url": discovered.canonical_url,
                    "full_text_available": n.full_text_available,
                    "access_level": n.access_level.as_str(),
                    "academic_status": n.academic_status.as_str(),
                    "epistemic": "bibliographic_resource",
                }),
            },
        )
        .await?;
        let after = talaria_store::count_corpus_snapshots(pool, corpus_id).await?;
        (snapshot_id, after > before)
    };
    link_corpus_snapshot(
        pool,
        corpus_id,
        snapshot_id,
        n.revision_token.as_deref(),
        &content_hash_key,
    )
    .await?;

    Ok((corpus_id, snapshot_id, snapshot_new))
}

async fn upsert_corpus_from_normalized(
    pool: &sqlx::PgPool,
    n: &NormalizedCorpusDocument,
) -> anyhow::Result<Uuid> {
    let (corpus_id, _) = upsert_corpus_document(
        pool,
        &CorpusDocumentInsert {
            source_kind: n.source_kind.as_str().into(),
            external_id: n.external_id.clone(),
            canonical_url: n.canonical_url.clone(),
            document_type: n.document_type.as_str().into(),
            title: n.title.clone(),
            language: n.language.clone(),
            abstract_text: n.abstract_text.clone(),
            academic_status: n.academic_status.as_str().into(),
            access_level: n.access_level.as_str().into(),
            full_text_available: n.full_text_available,
            rights_uri: n.rights_uri.clone(),
            rights_holder: n.rights_holder.clone(),
            rights_normalized: n.rights_normalized.as_str().into(),
            publisher_or_institution: n.publisher_or_institution.clone(),
            publication_time: typed_time_json(&n.publication_time),
            connector_version: n.connector_version.clone(),
        },
    )
    .await?;

    let idents: Vec<(String, String, String)> = n
        .identifiers
        .iter()
        .map(|i| {
            (
                i.scheme.as_str().to_string(),
                i.value_raw.clone(),
                i.value_normalized.clone(),
            )
        })
        .collect();
    replace_document_identifiers(pool, corpus_id, &idents).await?;

    let contribs: Vec<talaria_store::ContributionInsert> = n
        .contributions
        .iter()
        .map(|c| talaria_store::ContributionInsert {
            role: c.role.as_str().to_string(),
            agent_name: c.agent_name.clone(),
            name_normalized: c.name_normalized.clone(),
            identifier_scheme: c.identifier_scheme.map(|s| s.as_str().to_string()),
            identifier_value: c.identifier_value.clone(),
            ordinal: c.ordinal,
        })
        .collect();
    replace_document_contributions(pool, corpus_id, &contribs).await?;

    let subjects: Vec<talaria_store::SubjectInsert> = n
        .subjects
        .iter()
        .map(|s| talaria_store::SubjectInsert {
            scheme: s.scheme.clone(),
            label: s.label.clone(),
            identifier: s.identifier.clone(),
        })
        .collect();
    replace_document_subjects(pool, corpus_id, &subjects).await?;
    Ok(corpus_id)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_empty_providers_include_hal_persee_and_gallica() {
        let kinds = resolve_corpus_providers(&[], true);
        for expected in [
            SourceKind::Hal,
            SourceKind::Persee,
            SourceKind::Gallica,
            SourceKind::ThesesFr,
            SourceKind::OpenAlex,
            SourceKind::Bnf,
            SourceKind::OpenLibrary,
            SourceKind::InternetArchive,
            SourceKind::Europeana,
        ] {
            assert!(
                wants(&kinds, expected.clone()),
                "missing {}",
                expected.as_str()
            );
        }
        assert!(
            !wants(&kinds, SourceKind::Wikisource),
            "agora catalogs stay bibliographic"
        );
    }

    #[test]
    fn explorer_fact_providers_include_sister_wikis() {
        let names = explorer_fact_providers();
        assert!(names.iter().any(|n| n == "wikisource"));
        assert!(names.iter().any(|n| n == "wikimedia_commons"));
        assert!(
            !names.iter().any(|n| n == "hal"),
            "explorer must not pull Agora catalogs"
        );
    }

    #[test]
    fn fixture_empty_providers_stay_theses_fr() {
        let kinds = resolve_corpus_providers(&[], false);
        assert_eq!(kinds, vec![SourceKind::ThesesFr]);
    }

    #[test]
    fn explicit_providers_are_respected() {
        let kinds = resolve_corpus_providers(&["hal".into(), "persee".into()], true);
        assert_eq!(kinds, vec![SourceKind::Hal, SourceKind::Persee]);
    }

    #[test]
    fn provider_budget_reserves_a_minimum_for_later_sources() {
        let limits = CorpusIngestLimits {
            per_provider: 15,
            total: Some(20),
            minimum_per_provider: 3,
            provider_timeout: Duration::from_secs(1),
        };
        assert_eq!(provider_budget(limits, 3, 0, 0), 14);
        assert_eq!(provider_budget(limits, 3, 1, 14), 3);
        assert_eq!(provider_budget(limits, 3, 2, 17), 3);
    }

    #[test]
    fn provider_budget_divides_a_too_small_total_fairly() {
        let limits = CorpusIngestLimits {
            per_provider: 15,
            total: Some(5),
            minimum_per_provider: 3,
            provider_timeout: Duration::from_secs(1),
        };
        assert_eq!(provider_budget(limits, 3, 0, 0), 3);
        assert_eq!(provider_budget(limits, 3, 1, 3), 1);
        assert_eq!(provider_budget(limits, 3, 2, 4), 1);
    }

    use std::sync::atomic::{AtomicUsize, Ordering};
    use talaria_sources::{
        ConnectorError, ConnectorHealth, DiscoveryMethod, DiscoveryPage,
        DocumentType, SourceMetadata,
    };

    struct TimedFixtureConnector {
        active: Arc<AtomicUsize>,
        peak: Arc<AtomicUsize>,
        delay: Duration,
    }

    #[async_trait::async_trait]
    impl SourceConnector for TimedFixtureConnector {
        fn source_kind(&self) -> SourceKind {
            SourceKind::Fixture
        }

        fn connector_version(&self) -> &str {
            "timed-fixture:v1"
        }

        async fn discover(
            &self,
            _subject: &ResolvedSubject,
            _cursor: Option<DiscoveryCursor>,
        ) -> Result<DiscoveryPage, ConnectorError> {
            Ok(DiscoveryPage { documents: vec![], next_cursor: None })
        }

        async fn fetch(
            &self,
            document: &DiscoveredDocument,
        ) -> Result<FetchedDocument, ConnectorError> {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(active, Ordering::SeqCst);
            tokio::time::sleep(self.delay).await;
            self.active.fetch_sub(1, Ordering::SeqCst);
            if document.external_id == "429" {
                return Err(ConnectorError::RateLimited);
            }
            Ok(FetchedDocument {
                discovered: document.clone(),
                revision_id: Some("fixture-r1".into()),
                content_type: "application/json".into(),
                text: String::new(),
                raw_metadata: serde_json::json!({}),
                license: None,
                content_bytes: 0,
            })
        }

        async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
            Ok(ConnectorHealth { ok: true, detail: "fixture".into() })
        }
    }

    fn test_document(id: &str) -> DiscoveredDocument {
        DiscoveredDocument {
            source_kind: SourceKind::Fixture,
            external_id: id.into(),
            canonical_url: None,
            title: format!("Document {id}"),
            language: Some("en".into()),
            document_type: DocumentType::Article,
            subject_links: vec![],
            publication_time: None,
            discovery_method: DiscoveryMethod::Fixture,
            relevance_score: 1.0,
            source_metadata: SourceMetadata::default(),
        }
    }

    #[tokio::test]
    async fn bounded_fetches_preserve_discovery_order() {
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let connector: Arc<dyn SourceConnector> = Arc::new(TimedFixtureConnector {
            active: Arc::clone(&active),
            peak: Arc::clone(&peak),
            delay: Duration::from_millis(35),
        });
        let docs = (0..3)
            .map(|i| (Uuid::new_v4(), test_document(&i.to_string())))
            .collect();
        let result = fetch_document_batch(connector, docs, Duration::from_secs(2)).await.unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(peak.load(Ordering::SeqCst), 3);
        assert_eq!(result.iter().map(|(_, doc, _)| doc.external_id.as_str()).collect::<Vec<_>>(),
                   vec!["0", "1", "2"]);
        assert!(result.iter().all(|(_, _, value)| value.is_ok()));
    }

    #[tokio::test]
    async fn fetch_timeout_and_rate_limit_are_distinguishable() {
        let connector: Arc<dyn SourceConnector> = Arc::new(TimedFixtureConnector {
            active: Arc::new(AtomicUsize::new(0)),
            peak: Arc::new(AtomicUsize::new(0)),
            delay: Duration::from_millis(40),
        });
        let timed = fetch_document_batch(
            Arc::clone(&connector),
            vec![(Uuid::new_v4(), test_document("slow"))],
            Duration::from_millis(2),
        ).await.unwrap();
        assert!(timed[0].2.as_ref().unwrap_err().contains("timed out"));

        let limited = fetch_document_batch(
            connector,
            vec![(Uuid::new_v4(), test_document("429"))],
            Duration::from_secs(1),
        ).await.unwrap();
        assert_eq!(connector_failure_status(limited[0].2.as_ref().unwrap_err()), "rate_limited");
    }

    #[test]
    fn concurrency_is_bounded_and_defaults_to_two() {
        assert_eq!(resolved_fetch_concurrency(None), 2);
        assert_eq!(resolved_fetch_concurrency(Some("0")), 1);
        assert_eq!(resolved_fetch_concurrency(Some("99")), 4);
        assert_eq!(resolved_fetch_concurrency(Some("3")), 3);
        assert_eq!(resolved_fetch_concurrency(Some("invalid")), 2);
    }

    #[test]
    fn detects_stalled_discovery_cursor() {
        let cursor = Some(DiscoveryCursor { token: Some("page".into()), offset: 20 });
        assert!(cursor_stalled(&cursor, &cursor));
        assert!(!cursor_stalled(
            &cursor, &Some(DiscoveryCursor { token: Some("next".into()), offset: 40 }),
        ));
        assert!(!cursor_stalled(&None, &None));
    }

    #[test]
    fn provider_status_distinguishes_rate_limits() {
        assert_eq!(connector_failure_status("HTTP 429 Too Many Requests"), "rate_limited");
        assert_eq!(connector_failure_status("connection reset"), "failed");
    }
}
