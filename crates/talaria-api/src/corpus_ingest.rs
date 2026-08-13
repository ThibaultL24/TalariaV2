// crates/talaria-api/src/corpus_ingest.rs
//! Corpus ingest orchestration (PR1): discover → fetch → snapshot → normalize → persist.
//! Does NOT create quality claims, soft claims, events, or historiography positions.

use std::path::PathBuf;

use talaria_core::AppConfig;
use talaria_sources::connectors::{
    default_registry_with_corpus, BnfConfig, BnfConnector, CorpusConnectors, EuropeanaConfig,
    EuropeanaConnector, InternetArchiveConfig, InternetArchiveConnector, OpenAlexConfig,
    OpenAlexConnector, ThesesFrConfig, ThesesFrConnector,
};
use talaria_sources::{
    match_subject_to_document, normalize_bnf_notice, normalize_europeana_item, normalize_ia_item,
    normalize_openalex_work, normalize_these_detail, AccessLevel, DiscoveredDocument,
    NormalizedCorpusDocument, ResolvedSubject, SourceKind, TypedTimeLite,
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

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct CorpusIngestMetrics {
    pub documents_discovered: u64,
    pub documents_persisted: u64,
    pub documents_skipped: u64,
    pub snapshots_created: u64,
    pub snapshots_reused: u64,
    pub entity_links: u64,
    pub connector_errors: u64,
}

pub async fn run_corpus_ingest(
    config: &AppConfig,
    subject_label: &str,
    qid: Option<&str>,
    providers: &[String],
    limit: u32,
    use_fixture: bool,
    fixture_dir: Option<PathBuf>,
    live: bool,
) -> anyhow::Result<String> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;

    let subject_id =
        upsert_entity_with_kind(&pool, &config.wiki_lang, subject_label, "person").await?;
    let subject = ResolvedSubject {
        entity_id: Some(subject_id),
        qid: qid.map(str::to_string),
        label: subject_label.into(),
        languages: vec!["fr".into(), "en".into()],
        birth_year: None,
        death_year: None,
        countries: vec!["France".into()],
        occupations: vec![],
        known_identifiers: vec![],
    };

    let want_theses = providers.is_empty()
        || providers
            .iter()
            .any(|p| SourceKind::parse(p) == SourceKind::ThesesFr);
    let want_openalex = providers
        .iter()
        .any(|p| SourceKind::parse(p) == SourceKind::OpenAlex);
    let want_ia = providers
        .iter()
        .any(|p| SourceKind::parse(p) == SourceKind::InternetArchive);
    let want_europeana = providers
        .iter()
        .any(|p| SourceKind::parse(p) == SourceKind::Europeana);
    let want_bnf = providers
        .iter()
        .any(|p| SourceKind::parse(p) == SourceKind::Bnf);

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

    let registry = default_registry_with_corpus(
        None,
        false,
        CorpusConnectors {
            theses_fr: theses,
            open_alex,
            internet_archive,
            europeana,
            bnf,
        },
    )?;
    let kinds: Vec<SourceKind> = if providers.is_empty() {
        vec![SourceKind::ThesesFr]
    } else {
        providers.iter().map(|p| SourceKind::parse(p)).collect()
    };

    let run_id = start_discovery_run(
        &pool,
        &DiscoveryRunInsert {
            subject_entity_id: subject_id,
            subject_qid: subject.qid.clone(),
            subject_label: subject_label.into(),
            plan_json: serde_json::json!({
                "mode": "corpus_ingest",
                "providers": kinds.iter().map(|k| k.as_str()).collect::<Vec<_>>(),
                "limit": limit,
                "fixture": use_fixture,
            }),
            budgets_json: serde_json::json!({ "limit": limit }),
            connector_versions: serde_json::json!({}),
        },
    )
    .await?;

    let mut metrics = CorpusIngestMetrics::default();
    let mut remaining = limit;

    for kind in kinds {
        if remaining == 0 {
            break;
        }
        let Some(reg) = registry.get(&kind) else {
            continue;
        };
        let Some(connector) = &reg.connector else {
            continue;
        };
        if !reg.implemented {
            tracing::warn!(source = kind.as_str(), "connector not implemented; skip");
            continue;
        }

        let mut cursor = None;
        loop {
            if remaining == 0 {
                break;
            }
            let page = match connector.discover(&subject, cursor.clone()).await {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, source = kind.as_str(), "discover failed");
                    metrics.connector_errors += 1;
                    break;
                }
            };

            for doc in page.documents {
                if remaining == 0 {
                    break;
                }
                metrics.documents_discovered += 1;
                let (discovered_id, _) =
                    upsert_discovered_document(&pool, &to_discovered_insert(run_id, &doc)).await?;

                let fetched = match connector.fetch(&doc).await {
                    Ok(f) => f,
                    Err(e) => {
                        tracing::warn!(error = %e, id = %doc.external_id, "fetch failed");
                        metrics.connector_errors += 1;
                        mark_discovered_skipped(&pool, discovered_id, "fetch_failed").await?;
                        metrics.documents_skipped += 1;
                        continue;
                    }
                };

                let normalized = extract_normalized(&kind, &fetched.raw_metadata)?;
                let (corpus_id, snapshot_id, snapshot_new) =
                    persist_normalized(&pool, &kind, &doc, &normalized).await?;

                mark_discovered_corpus_document(&pool, discovered_id, corpus_id).await?;
                mark_discovered_snapshotted(&pool, discovered_id, snapshot_id).await?;
                metrics.documents_persisted += 1;
                if snapshot_new {
                    metrics.snapshots_created += 1;
                } else {
                    metrics.snapshots_reused += 1;
                }

                if let Some(m) = match_subject_to_document(subject_label, &normalized) {
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

                remaining = remaining.saturating_sub(1);
            }

            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
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

fn extract_normalized(
    kind: &SourceKind,
    raw_metadata: &serde_json::Value,
) -> anyhow::Result<NormalizedCorpusDocument> {
    if let Some(n) = raw_metadata.get("normalized") {
        return Ok(serde_json::from_value(n.clone())?);
    }
    match kind {
        SourceKind::ThesesFr => {
            let provider = raw_metadata
                .get("provider")
                .cloned()
                .unwrap_or_else(|| raw_metadata.clone());
            Ok(normalize_these_detail(&provider)?)
        }
        SourceKind::OpenAlex => {
            let provider = raw_metadata
                .get("provider")
                .cloned()
                .unwrap_or_else(|| raw_metadata.clone());
            Ok(normalize_openalex_work(&provider)?)
        }
        SourceKind::InternetArchive => {
            let provider = raw_metadata
                .get("provider")
                .cloned()
                .unwrap_or_else(|| raw_metadata.clone());
            Ok(normalize_ia_item(&provider)?)
        }
        SourceKind::Europeana => {
            let provider = raw_metadata
                .get("provider")
                .cloned()
                .unwrap_or_else(|| raw_metadata.clone());
            Ok(normalize_europeana_item(&provider)?)
        }
        SourceKind::Bnf => {
            let provider = raw_metadata
                .get("provider")
                .cloned()
                .unwrap_or_else(|| raw_metadata.clone());
            Ok(normalize_bnf_notice(&provider)?)
        }
        other => anyhow::bail!("no normalizer for {}", other.as_str()),
    }
}

async fn persist_normalized(
    pool: &sqlx::PgPool,
    kind: &SourceKind,
    discovered: &DiscoveredDocument,
    n: &NormalizedCorpusDocument,
) -> anyhow::Result<(Uuid, Uuid, bool)> {
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

    let hash = n.content_fingerprint();
    let source_uri = n
        .canonical_url
        .clone()
        .unwrap_or_else(|| format!("{}:{}", kind.as_str(), n.external_id));
    // content_hash identity = fingerprint alone (already includes revision axes).
    let content_hash_key = hash.clone();

    // Detect reuse: same hash already linked?
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
            // Metadata-only / open metadata: store projection, never remote PDF bytes.
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
    link_corpus_snapshot(
        pool,
        corpus_id,
        snapshot_id,
        n.revision_token.as_deref(),
        &content_hash_key,
    )
    .await?;
    let after = talaria_store::count_corpus_snapshots(pool, corpus_id).await?;
    let snapshot_new = after > before;

    Ok((corpus_id, snapshot_id, snapshot_new))
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
