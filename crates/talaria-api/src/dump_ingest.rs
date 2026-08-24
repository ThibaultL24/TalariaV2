// crates/talaria-api/src/dump_ingest.rs
//! Lot B: dump file → document_snapshots + sentence fragments. No event extraction.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use talaria_core::AppConfig;
use talaria_dump::{hash_dump_file, DumpReader, DumpRecord, JsonlDumpReader};
use talaria_store::{
    connect, corpus_dump_document_status, corpus_dump_document_status_counts,
    count_sentence_fragments, find_document_snapshot, finish_corpus_dump_run,
    get_corpus_dump_run, insert_document_fragment, insert_document_snapshot,
    latest_corpus_dump_run, mark_corpus_dump_running, run_migrations, start_corpus_dump_run, update_corpus_dump_progress,
    upsert_corpus_dump_document, CorpusDumpDocumentUpsert, CorpusDumpRunInsert,
    DocumentFragmentInsert, DocumentSnapshotInsert,
};
use talaria_text::split_sentences;
use uuid::Uuid;

const TERMINAL_DOC_STATUSES: &[&str] = &["ingested", "skipped_unchanged", "failed", "filtered"];

#[derive(Debug, Clone, Serialize)]
pub struct DumpIngestReport {
    pub run_id: Option<Uuid>,
    pub dry_run: bool,
    pub dump_uri: String,
    pub content_hash: String,
    pub reader_id: String,
    pub reader_version: String,
    pub documents_read: u64,
    pub snapshots_created: u64,
    pub skipped_unchanged: u64,
    pub filtered: u64,
    pub failed: u64,
    pub fragments: u64,
    pub status: String,
}

impl DumpIngestReport {
    fn metrics_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({}))
    }
}

#[derive(Debug, Clone)]
pub struct DumpIngestOpts {
    pub file: PathBuf,
    pub source_kind: String,
    pub subject: Option<String>,
    pub language: Option<String>,
    pub dry_run: bool,
    pub skip_existing: bool,
    pub limit: usize,
    pub resume_run: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotSpec {
    pub source_type: String,
    pub source_uri: String,
    pub source_identifier: Option<String>,
    pub language: String,
    pub title: Option<String>,
    pub content_hash: String,
    pub text: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentSpec {
    pub text: String,
    pub start_offset: i32,
    pub end_offset: i32,
    pub ordinal: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordPlan {
    Filtered { reason: &'static str },
    Ready {
        snapshot: SnapshotSpec,
        fragments: Vec<FragmentSpec>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistResult {
    Created,
    Unchanged,
}

pub trait RecordSink {
    fn persist(
        &mut self,
        spec: &SnapshotSpec,
        fragments: &[FragmentSpec],
    ) -> anyhow::Result<PersistResult>;
}

pub fn text_hash(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

pub fn record_source_uri(record: &DumpRecord, source_kind: &str) -> String {
    record
        .canonical_url
        .clone()
        .unwrap_or_else(|| format!("{source_kind}:{}", record.external_id))
}

pub fn snapshot_spec(record: &DumpRecord, source_kind: &str) -> SnapshotSpec {
    SnapshotSpec {
        source_type: source_kind.to_string(),
        source_uri: record_source_uri(record, source_kind),
        source_identifier: Some(record.external_id.clone()),
        language: record
            .language
            .clone()
            .unwrap_or_else(|| "en".to_string()),
        title: record.title.clone(),
        content_hash: text_hash(&record.text),
        text: record.text.clone(),
        metadata: serde_json::json!({
            "document_type": record.document_type,
            "contributors": record.contributors,
            "external_ids": record.external_ids,
            "license": record.license,
            "published": record.published,
            "provider_metadata": record.provider_metadata,
        }),
    }
}

pub fn sentence_fragments(text: &str) -> Vec<FragmentSpec> {
    split_sentences(text)
        .into_iter()
        .map(|span| FragmentSpec {
            text: span.text,
            start_offset: span.char_start,
            end_offset: span.char_end,
            ordinal: span.ordinal,
        })
        .collect()
}

pub fn plan_record(
    record: &DumpRecord,
    source_kind: &str,
    subject: Option<&str>,
    language: Option<&str>,
) -> RecordPlan {
    if let Some(want) = language {
        if let Some(got) = record.language.as_deref() {
            if !got.eq_ignore_ascii_case(want) {
                return RecordPlan::Filtered {
                    reason: "language",
                };
            }
        }
    }
    if let Some(subject) = subject {
        let needle = subject.to_lowercase();
        let hay = format!(
            "{} {}",
            record.title.as_deref().unwrap_or(""),
            record.text
        )
        .to_lowercase();
        if !hay.contains(&needle) {
            return RecordPlan::Filtered {
                reason: "subject",
            };
        }
    }
    RecordPlan::Ready {
        snapshot: snapshot_spec(record, source_kind),
        fragments: planned_fragments(source_kind, &record.text),
    }
}

fn planned_fragments(source_kind: &str, text: &str) -> Vec<FragmentSpec> {
    if source_kind.eq_ignore_ascii_case("wikipedia")
        && crate::wiki_persist::looks_like_wikitext(text)
    {
        talaria_sources::fragment_inserts(Uuid::nil(), text)
            .into_iter()
            .map(|ins| FragmentSpec {
                text: ins.text,
                start_offset: ins.start_offset,
                end_offset: ins.end_offset,
                ordinal: ins.ordinal,
            })
            .collect()
    } else {
        sentence_fragments(text)
    }
}

pub fn ingest_dump_records(
    records: impl IntoIterator<Item = DumpRecord>,
    source_kind: &str,
    subject: Option<&str>,
    language: Option<&str>,
    sink: &mut dyn RecordSink,
) -> DumpIngestReport {
    let mut report = DumpIngestReport {
        run_id: None,
        dry_run: false,
        dump_uri: String::new(),
        content_hash: String::new(),
        reader_id: "memory".into(),
        reader_version: "1".into(),
        documents_read: 0,
        snapshots_created: 0,
        skipped_unchanged: 0,
        filtered: 0,
        failed: 0,
        fragments: 0,
        status: "completed".into(),
    };
    for record in records {
        report.documents_read += 1;
        match plan_record(&record, source_kind, subject, language) {
            RecordPlan::Filtered { .. } => report.filtered += 1,
            RecordPlan::Ready {
                snapshot,
                fragments,
            } => match sink.persist(&snapshot, &fragments) {
                Ok(PersistResult::Created) => {
                    report.snapshots_created += 1;
                    report.fragments += fragments.len() as u64;
                }
                Ok(PersistResult::Unchanged) => report.skipped_unchanged += 1,
                Err(_) => report.failed += 1,
            },
        }
    }
    report
}

fn dump_uri(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

pub async fn run_dump_plan(
    opts: &DumpIngestOpts,
) -> anyhow::Result<DumpIngestReport> {
    let mut dry = opts.clone();
    dry.dry_run = true;
    run_dump_ingest_with_config(None, &dry).await
}

pub async fn run_dump_ingest(
    config: &AppConfig,
    opts: &DumpIngestOpts,
) -> anyhow::Result<DumpIngestReport> {
    run_dump_ingest_with_config(Some(config), opts).await
}

pub async fn run_dump_resume(
    config: &AppConfig,
    run_id: Uuid,
    skip_existing: bool,
) -> anyhow::Result<DumpIngestReport> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let run = get_corpus_dump_run(&pool, run_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("corpus dump run not found: {run_id}"))?;
    let opts = DumpIngestOpts {
        file: PathBuf::from(&run.dump_uri),
        source_kind: run.source_kind,
        subject: None,
        language: None,
        dry_run: false,
        skip_existing,
        limit: 0,
        resume_run: Some(run_id),
    };
    run_dump_ingest_with_config(Some(config), &opts).await
}

pub async fn run_dump_status(
    config: &AppConfig,
    run_id: Option<Uuid>,
) -> anyhow::Result<serde_json::Value> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let run = match run_id {
        Some(id) => get_corpus_dump_run(&pool, id).await?,
        None => latest_corpus_dump_run(&pool).await?,
    };
    let Some(run) = run else {
        anyhow::bail!("no corpus dump run found");
    };
    let counts = corpus_dump_document_status_counts(&pool, run.id).await?;
    Ok(serde_json::json!({
        "run_id": run.id,
        "source_kind": run.source_kind,
        "dump_uri": run.dump_uri,
        "content_hash": run.content_hash,
        "reader_id": run.reader_id,
        "reader_version": run.reader_version,
        "status": run.status,
        "cursor": run.cursor_json,
        "metrics": run.metrics_json,
        "error": run.error,
        "started_at": run.started_at,
        "ended_at": run.ended_at,
        "documents": counts.into_iter().map(|(status, n)| serde_json::json!({
            "status": status,
            "count": n
        })).collect::<Vec<_>>(),
    }))
}

async fn run_dump_ingest_with_config(
    config: Option<&AppConfig>,
    opts: &DumpIngestOpts,
) -> anyhow::Result<DumpIngestReport> {
    let dump_uri = dump_uri(&opts.file);
    let content_hash = hash_dump_file(&opts.file)?;
    let mut reader = JsonlDumpReader::open(&opts.file)?;
    let mut report = DumpIngestReport {
        run_id: opts.resume_run,
        dry_run: opts.dry_run,
        dump_uri: dump_uri.clone(),
        content_hash: content_hash.clone(),
        reader_id: reader.reader_id().to_string(),
        reader_version: reader.version().to_string(),
        documents_read: 0,
        snapshots_created: 0,
        skipped_unchanged: 0,
        filtered: 0,
        failed: 0,
        fragments: 0,
        status: if opts.dry_run {
            "planned".into()
        } else {
            "running".into()
        },
    };

    if opts.dry_run || config.is_none() {
        drive_reader(&mut reader, opts, &mut report)?;
        report.status = "planned".into();
        return Ok(report);
    }

    let config = config.expect("postgres ingest requires config");
    let pool = connect(config).await?;
    run_migrations(&pool).await?;

    let run_id = if let Some(id) = opts.resume_run {
        let existing = get_corpus_dump_run(&pool, id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("corpus dump run not found: {id}"))?;
        if existing.content_hash != content_hash {
            anyhow::bail!(
                "dump file hash changed since run {id} (stored {}, now {})",
                existing.content_hash,
                content_hash
            );
        }
        if existing.cursor_json != serde_json::json!({}) {
            reader.restore(&existing.cursor_json)?;
        }
        report.documents_read = existing
            .metrics_json
            .get("documents_read")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        report.snapshots_created = existing
            .metrics_json
            .get("snapshots_created")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        report.skipped_unchanged = existing
            .metrics_json
            .get("skipped_unchanged")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        report.filtered = existing
            .metrics_json
            .get("filtered")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        report.failed = existing
            .metrics_json
            .get("failed")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        report.fragments = existing
            .metrics_json
            .get("fragments")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        mark_corpus_dump_running(&pool, id).await?;
        id
    } else {
        start_corpus_dump_run(
            &pool,
            &CorpusDumpRunInsert {
                source_kind: opts.source_kind.clone(),
                dump_uri: dump_uri.clone(),
                content_hash: content_hash.clone(),
                reader_id: report.reader_id.clone(),
                reader_version: report.reader_version.clone(),
            },
        )
        .await?
    };
    report.run_id = Some(run_id);

    let reached_eof = persist_from_reader(&pool, &mut reader, opts, run_id, &mut report).await?;
    let cursor = reader.checkpoint();
    if reached_eof {
        report.status = "completed".into();
        finish_corpus_dump_run(
            &pool,
            run_id,
            "completed",
            &cursor,
            &report.metrics_json(),
            None,
        )
        .await?;
    } else {
        report.status = "running".into();
        update_corpus_dump_progress(&pool, run_id, &cursor, &report.metrics_json()).await?;
    }
    Ok(report)
}

fn drive_reader(
    reader: &mut dyn DumpReader,
    opts: &DumpIngestOpts,
    report: &mut DumpIngestReport,
) -> anyhow::Result<bool> {
    let mut reached_eof = true;
    loop {
        if opts.limit > 0 && report.documents_read >= opts.limit as u64 {
            reached_eof = false;
            break;
        }
        let Some(record) = reader.next_record()? else {
            break;
        };
        report.documents_read += 1;
        match plan_record(
            &record,
            &opts.source_kind,
            opts.subject.as_deref(),
            opts.language.as_deref(),
        ) {
            RecordPlan::Filtered { .. } => report.filtered += 1,
            RecordPlan::Ready { fragments, .. } => {
                report.snapshots_created += 1;
                report.fragments += fragments.len() as u64;
            }
        }
    }
    Ok(reached_eof)
}

async fn persist_from_reader(
    pool: &sqlx::PgPool,
    reader: &mut JsonlDumpReader,
    opts: &DumpIngestOpts,
    run_id: Uuid,
    report: &mut DumpIngestReport,
) -> anyhow::Result<bool> {
    let mut reached_eof = true;
    let baseline_read = report.documents_read;
    loop {
        if opts.limit > 0 && report.documents_read.saturating_sub(baseline_read) >= opts.limit as u64
        {
            reached_eof = false;
            break;
        }
        let Some(record) = reader.next_record()? else {
            break;
        };
        report.documents_read += 1;
        let cursor = reader.checkpoint();
        let byte_offset = cursor.get("byte_offset").and_then(|v| v.as_i64());

        let already_done = if opts.skip_existing {
            corpus_dump_document_status(pool, run_id, &record.external_id)
                .await?
                .map(|status| TERMINAL_DOC_STATUSES.contains(&status.as_str()))
                .unwrap_or(false)
        } else {
            false
        };

        if !already_done {
            let outcome = persist_one_document(pool, opts, run_id, &record, byte_offset).await;
            match outcome {
                Ok(PersistOutcome::Filtered) => report.filtered += 1,
                Ok(PersistOutcome::Created { fragments }) => {
                    report.snapshots_created += 1;
                    report.fragments += fragments;
                }
                Ok(PersistOutcome::Unchanged) => report.skipped_unchanged += 1,
                Err(err) => {
                    report.failed += 1;
                    tracing::warn!(
                        external_id = %record.external_id,
                        error = %err,
                        "dump document failed; continuing"
                    );
                    let _ = upsert_corpus_dump_document(
                        pool,
                        &CorpusDumpDocumentUpsert {
                            run_id,
                            external_id: record.external_id.clone(),
                            snapshot_id: None,
                            content_hash: Some(text_hash(&record.text)),
                            status: "failed".into(),
                            error: Some(err.to_string()),
                            byte_offset,
                        },
                    )
                    .await;
                }
            }
        }
        update_corpus_dump_progress(pool, run_id, &cursor, &report.metrics_json()).await?;
    }
    Ok(reached_eof)
}

enum PersistOutcome {
    Filtered,
    Created { fragments: u64 },
    Unchanged,
}

async fn persist_one_document(
    pool: &sqlx::PgPool,
    opts: &DumpIngestOpts,
    run_id: Uuid,
    record: &DumpRecord,
    byte_offset: Option<i64>,
) -> anyhow::Result<PersistOutcome> {
    match plan_record(
        record,
        &opts.source_kind,
        opts.subject.as_deref(),
        opts.language.as_deref(),
    ) {
        RecordPlan::Filtered { reason } => {
            upsert_corpus_dump_document(
                pool,
                &CorpusDumpDocumentUpsert {
                    run_id,
                    external_id: record.external_id.clone(),
                    snapshot_id: None,
                    content_hash: Some(text_hash(&record.text)),
                    status: "filtered".into(),
                    error: Some(reason.into()),
                    byte_offset,
                },
            )
            .await?;
            Ok(PersistOutcome::Filtered)
        }
        RecordPlan::Ready {
            snapshot,
            fragments,
        } => {
            let existed = find_document_snapshot(
                pool,
                &snapshot.source_type,
                &snapshot.source_uri,
                &snapshot.content_hash,
            )
            .await?;
            let snapshot_id = insert_document_snapshot(
                pool,
                &DocumentSnapshotInsert {
                    source_type: snapshot.source_type.clone(),
                    source_uri: snapshot.source_uri.clone(),
                    source_identifier: snapshot.source_identifier.clone(),
                    language: snapshot.language.clone(),
                    title: snapshot.title.clone(),
                    content_hash: snapshot.content_hash.clone(),
                    revision_id: None,
                    wiki_page_id: None,
                    raw_document_id: None,
                    text: snapshot.text.clone(),
                    metadata: snapshot.metadata.clone(),
                },
            )
            .await?;
            let existing_frags = count_sentence_fragments(pool, snapshot_id).await?;
            if existing_frags == 0 {
                let wiki_ok = opts.source_kind.eq_ignore_ascii_case("wikipedia")
                    && crate::wiki_persist::looks_like_wikitext(&snapshot.text)
                    && crate::wiki_persist::persist_wiki_fragments(
                        pool,
                        snapshot_id,
                        &snapshot.text,
                    )
                    .await
                    .is_ok();
                let still_empty = count_sentence_fragments(pool, snapshot_id).await? == 0;
                if !wiki_ok && still_empty {
                    for frag in &fragments {
                        insert_document_fragment(
                            pool,
                            &DocumentFragmentInsert {
                                snapshot_id,
                                fragment_kind: "sentence".into(),
                                parent_fragment_id: None,
                                sentence_id: None,
                                text: frag.text.clone(),
                                start_offset: frag.start_offset,
                                end_offset: frag.end_offset,
                                clause_index: None,
                                ordinal: frag.ordinal,
                                metadata: serde_json::json!({}),
                            },
                        )
                        .await?;
                    }
                }
            }
            let unchanged = existed.is_some() && existing_frags > 0;
            let status = if unchanged {
                "skipped_unchanged"
            } else {
                "ingested"
            };
            upsert_corpus_dump_document(
                pool,
                &CorpusDumpDocumentUpsert {
                    run_id,
                    external_id: record.external_id.clone(),
                    snapshot_id: Some(snapshot_id),
                    content_hash: Some(snapshot.content_hash.clone()),
                    status: status.into(),
                    error: None,
                    byte_offset,
                },
            )
            .await?;
            if unchanged {
                Ok(PersistOutcome::Unchanged)
            } else {
                Ok(PersistOutcome::Created {
                    fragments: fragments.len() as u64,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::Write;

    fn rec(id: &str, title: &str, text: &str) -> DumpRecord {
        DumpRecord {
            external_id: id.into(),
            title: Some(title.into()),
            text: text.into(),
            language: Some("en".into()),
            canonical_url: None,
            document_type: "encyclopedia_article".into(),
            published: None,
            contributors: vec![],
            external_ids: vec![],
            license: None,
            provider_metadata: serde_json::json!({}),
        }
    }

    struct MemSink {
        snapshots: HashMap<(String, String, String), Uuid>,
        fragments: HashMap<Uuid, usize>,
        fail_ids: Vec<String>,
    }

    impl MemSink {
        fn new() -> Self {
            Self {
                snapshots: HashMap::new(),
                fragments: HashMap::new(),
                fail_ids: vec![],
            }
        }
    }

    impl RecordSink for MemSink {
        fn persist(
            &mut self,
            spec: &SnapshotSpec,
            fragments: &[FragmentSpec],
        ) -> anyhow::Result<PersistResult> {
            if self
                .fail_ids
                .iter()
                .any(|id| spec.source_identifier.as_deref() == Some(id.as_str()))
            {
                anyhow::bail!("forced fail");
            }
            let key = (
                spec.source_type.clone(),
                spec.source_uri.clone(),
                spec.content_hash.clone(),
            );
            if let Some(id) = self.snapshots.get(&key) {
                if self.fragments.get(id).copied().unwrap_or(0) > 0 {
                    return Ok(PersistResult::Unchanged);
                }
            }
            let id = Uuid::new_v4();
            self.snapshots.insert(key, id);
            self.fragments.insert(id, fragments.len());
            Ok(PersistResult::Created)
        }
    }

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/dumps/mini_events.jsonl")
    }

    #[test]
    fn wikipedia_wikitext_plan_emits_more_fragments_than_plain_sentences() {
        let wikitext = "== A ==\nFirst sentence is long enough.\n\n== B ==\nSecond sentence is also long enough.\n";
        let nap = rec("n", "Napoleon", wikitext);
        let sentence_n = sentence_fragments(wikitext).len();
        match plan_record(&nap, "wikipedia", None, None) {
            RecordPlan::Ready { fragments, .. } => {
                assert!(
                    fragments.len() > sentence_n,
                    "expected section+sentence fragments, got {} vs sentence-only {}",
                    fragments.len(),
                    sentence_n
                );
            }
            other => panic!("expected Ready, got {other:?}"),
        }
        match plan_record(
            &rec(
                "p",
                "Napoleon",
                "Napoleon Bonaparte was born on 15 August 1769 in Ajaccio.",
            ),
            "jsonl",
            None,
            None,
        ) {
            RecordPlan::Ready { fragments, .. } => {
                assert_eq!(
                    fragments.len(),
                    sentence_fragments(
                        "Napoleon Bonaparte was born on 15 August 1769 in Ajaccio."
                    )
                    .len()
                );
            }
            other => panic!("expected Ready, got {other:?}"),
        }
    }

    #[test]
    fn napoleon_bio_splits_into_sentences_not_whole_doc() {
        let text = "Napoleon Bonaparte was born on 15 August 1769 in Ajaccio. He died on 5 May 1821 on Saint Helena.";
        let frags = sentence_fragments(text);
        assert!(frags.len() >= 2, "got {:?}", frags);
        assert!(frags.iter().all(|f| f.text.len() < text.len()));
        assert_eq!(frags[0].ordinal, 0);
        assert!(frags[0].end_offset <= frags[1].start_offset);
    }

    #[test]
    fn subject_filter_drops_curie_keeps_napoleon() {
        let nap = rec(
            "n",
            "Napoleon",
            "Napoleon Bonaparte was born on 15 August 1769 in Ajaccio.",
        );
        let curie = rec(
            "c",
            "Marie Curie",
            "Marie Curie discovered radium in Paris in 1898.",
        );
        assert!(matches!(
            plan_record(&nap, "jsonl", Some("Napoleon"), None),
            RecordPlan::Ready { .. }
        ));
        assert!(matches!(
            plan_record(&curie, "jsonl", Some("Napoleon"), None),
            RecordPlan::Filtered { reason: "subject" }
        ));
    }

    #[test]
    fn hash_idempotence_second_persist_is_unchanged() {
        let docs = vec![rec(
            "a",
            "Ada",
            "Ada Lovelace was born in 1815 in London.",
        )];
        let mut sink = MemSink::new();
        let first = ingest_dump_records(docs.clone(), "jsonl", None, None, &mut sink);
        let second = ingest_dump_records(docs, "jsonl", None, None, &mut sink);
        assert_eq!(first.snapshots_created, 1);
        assert_eq!(first.failed, 0);
        assert_eq!(second.snapshots_created, 0);
        assert_eq!(second.skipped_unchanged, 1);
        assert_eq!(sink.snapshots.len(), 1);
    }

    #[test]
    fn one_failed_document_does_not_stop_the_run() {
        let docs = vec![
            rec("a", "A", "Ada Lovelace was born in 1815 in London."),
            rec("b", "B", "This document will fail on persist purpose."),
            rec("c", "C", "Marie Curie discovered radium in Paris in 1898."),
        ];
        let mut sink = MemSink::new();
        sink.fail_ids.push("b".into());
        let report = ingest_dump_records(docs, "jsonl", None, None, &mut sink);
        assert_eq!(report.documents_read, 3);
        assert_eq!(report.snapshots_created, 2);
        assert_eq!(report.failed, 1);
        assert_eq!(sink.snapshots.len(), 2);
    }

    #[test]
    fn dry_plan_reads_fixture_without_run_id() {
        let opts = DumpIngestOpts {
            file: fixture_path(),
            source_kind: "jsonl".into(),
            subject: None,
            language: None,
            dry_run: true,
            skip_existing: false,
            limit: 0,
            resume_run: None,
        };
        let report = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(run_dump_plan(&opts))
            .unwrap();
        assert!(report.dry_run);
        assert_eq!(report.documents_read, 8);
        assert!(report.fragments >= 8);
        assert!(report.run_id.is_none());
        assert_eq!(report.status, "planned");
    }

    #[test]
    fn jsonl_reader_resume_continues_after_limit() {
        let mut reader = JsonlDumpReader::open(fixture_path()).unwrap();
        let _ = reader.next_record().unwrap();
        let _ = reader.next_record().unwrap();
        let cursor = reader.checkpoint();
        let mut resumed = JsonlDumpReader::open(fixture_path()).unwrap();
        resumed.restore(&cursor).unwrap();
        let next = resumed.next_record().unwrap().unwrap();
        assert_eq!(next.external_id, "fixture:amiens");
    }

    struct ForceFailSink;

    impl RecordSink for ForceFailSink {
        fn persist(
            &mut self,
            spec: &SnapshotSpec,
            _fragments: &[FragmentSpec],
        ) -> anyhow::Result<PersistResult> {
            if spec.source_identifier.as_deref() == Some("poison") {
                anyhow::bail!("db down");
            }
            Ok(PersistResult::Created)
        }
    }

    #[test]
    fn persist_error_is_isolated_to_that_document() {
        let docs = vec![
            rec("ok1", "Ok", "Napoleon fought at Waterloo in 1815 near Waterloo."),
            rec("poison", "Bad", "This row should fail while others continue."),
            rec("ok2", "Ok2", "In 1814 Napoleon was exiled to Elba."),
        ];
        let mut sink = ForceFailSink;
        let report = ingest_dump_records(docs, "jsonl", None, None, &mut sink);
        assert_eq!(report.failed, 1);
        assert_eq!(report.snapshots_created, 2);
        assert_eq!(report.status, "completed");
    }

    fn test_config() -> AppConfig {
        let root_env = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.env");
        let _ = dotenvy::from_path(&root_env);
        dotenvy::dotenv().ok();
        AppConfig::from_env().expect("DATABASE_URL")
    }

    #[tokio::test]
    async fn postgres_ingest_is_idempotent_on_content_hash() {
        let config = test_config();
        let kind = format!("jsonl-test-{}", Uuid::new_v4());
        let opts = DumpIngestOpts {
            file: fixture_path(),
            source_kind: kind.clone(),
            subject: None,
            language: None,
            dry_run: false,
            skip_existing: false,
            limit: 0,
            resume_run: None,
        };
        let first = run_dump_ingest(&config, &opts).await.unwrap();
        let second = run_dump_ingest(&config, &opts).await.unwrap();
        assert_eq!(first.documents_read, 8);
        assert_eq!(first.snapshots_created, 8);
        assert_eq!(first.failed, 0);
        assert!(first.fragments >= 8);
        assert_eq!(second.snapshots_created, 0);
        assert_eq!(second.skipped_unchanged, 8);
        assert!(first.run_id.is_some());
        assert_ne!(first.run_id, second.run_id);
    }

    #[tokio::test]
    async fn postgres_resume_continues_from_cursor() {
        let config = test_config();
        let kind = format!("jsonl-test-{}", Uuid::new_v4());
        let dir = tempfile::tempdir().unwrap();
        let copy = dir.path().join("mini.jsonl");
        std::fs::copy(fixture_path(), &copy).unwrap();

        let first_opts = DumpIngestOpts {
            file: copy.clone(),
            source_kind: kind,
            subject: None,
            language: None,
            dry_run: false,
            skip_existing: true,
            limit: 3,
            resume_run: None,
        };
        let first = run_dump_ingest(&config, &first_opts).await.unwrap();
        assert_eq!(first.documents_read, 3);
        assert_eq!(first.snapshots_created, 3);
        assert_eq!(first.status, "running");

        let resumed = run_dump_resume(&config, first.run_id.unwrap(), true)
            .await
            .unwrap();
        assert_eq!(resumed.documents_read, 8);
        assert_eq!(resumed.snapshots_created, 8);
        assert_eq!(resumed.failed, 0);
        assert_eq!(resumed.status, "completed");
    }

    #[tokio::test]
    async fn postgres_one_invalid_line_does_not_abort_run() {
        let config = test_config();
        let kind = format!("jsonl-test-{}", Uuid::new_v4());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mixed.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"external_id":"keep-a","text":"Ada Lovelace was born in 1815 in London."}}"#
        )
        .unwrap();
        writeln!(f, "not-json").unwrap();
        writeln!(
            f,
            r#"{{"external_id":"keep-b","text":"Marie Curie discovered radium in Paris in 1898."}}"#
        )
        .unwrap();

        let opts = DumpIngestOpts {
            file: path,
            source_kind: kind,
            subject: None,
            language: None,
            dry_run: false,
            skip_existing: false,
            limit: 0,
            resume_run: None,
        };
        let report = run_dump_ingest(&config, &opts).await.unwrap();
        assert_eq!(report.documents_read, 2);
        assert_eq!(report.snapshots_created, 2);
        assert_eq!(report.status, "completed");
    }
}
