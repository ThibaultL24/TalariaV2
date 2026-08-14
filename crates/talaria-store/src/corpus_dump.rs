// crates/talaria-store/src/corpus_dump.rs
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CorpusDumpRunInsert {
    pub source_kind: String,
    pub dump_uri: String,
    pub content_hash: String,
    pub reader_id: String,
    pub reader_version: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CorpusDumpRunRow {
    pub id: Uuid,
    pub source_kind: String,
    pub dump_uri: String,
    pub content_hash: String,
    pub reader_id: String,
    pub reader_version: String,
    pub status: String,
    pub cursor_json: serde_json::Value,
    pub metrics_json: serde_json::Value,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

pub async fn start_corpus_dump_run(
    pool: &PgPool,
    run: &CorpusDumpRunInsert,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO corpus_dump_runs (
            source_kind, dump_uri, content_hash, reader_id, reader_version, status
        )
        VALUES ($1,$2,$3,$4,$5,'running')
        RETURNING id
        "#,
    )
    .bind(&run.source_kind)
    .bind(&run.dump_uri)
    .bind(&run.content_hash)
    .bind(&run.reader_id)
    .bind(&run.reader_version)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn get_corpus_dump_run(
    pool: &PgPool,
    run_id: Uuid,
) -> anyhow::Result<Option<CorpusDumpRunRow>> {
    let row = sqlx::query_as::<_, CorpusDumpRunRow>(
        r#"
        SELECT id, source_kind, dump_uri, content_hash, reader_id, reader_version,
               status, cursor_json, metrics_json, error, started_at, ended_at
        FROM corpus_dump_runs
        WHERE id = $1
        "#,
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn latest_corpus_dump_run(pool: &PgPool) -> anyhow::Result<Option<CorpusDumpRunRow>> {
    let row = sqlx::query_as::<_, CorpusDumpRunRow>(
        r#"
        SELECT id, source_kind, dump_uri, content_hash, reader_id, reader_version,
               status, cursor_json, metrics_json, error, started_at, ended_at
        FROM corpus_dump_runs
        ORDER BY started_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn mark_corpus_dump_running(pool: &PgPool, run_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE corpus_dump_runs
        SET status = 'running', ended_at = NULL, error = NULL
        WHERE id = $1
        "#,
    )
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_corpus_dump_progress(
    pool: &PgPool,
    run_id: Uuid,
    cursor: &serde_json::Value,
    metrics: &serde_json::Value,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE corpus_dump_runs
        SET cursor_json = $2, metrics_json = $3
        WHERE id = $1
        "#,
    )
    .bind(run_id)
    .bind(cursor)
    .bind(metrics)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn finish_corpus_dump_run(
    pool: &PgPool,
    run_id: Uuid,
    status: &str,
    cursor: &serde_json::Value,
    metrics: &serde_json::Value,
    error: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE corpus_dump_runs
        SET status = $2, cursor_json = $3, metrics_json = $4, error = $5, ended_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(run_id)
    .bind(status)
    .bind(cursor)
    .bind(metrics)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct CorpusDumpDocumentUpsert {
    pub run_id: Uuid,
    pub external_id: String,
    pub snapshot_id: Option<Uuid>,
    pub content_hash: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub byte_offset: Option<i64>,
}

pub async fn upsert_corpus_dump_document(
    pool: &PgPool,
    doc: &CorpusDumpDocumentUpsert,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO corpus_dump_documents (
            run_id, external_id, snapshot_id, content_hash, status, error, byte_offset
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7)
        ON CONFLICT (run_id, external_id) DO UPDATE SET
            snapshot_id = EXCLUDED.snapshot_id,
            content_hash = EXCLUDED.content_hash,
            status = EXCLUDED.status,
            error = EXCLUDED.error,
            byte_offset = EXCLUDED.byte_offset,
            updated_at = NOW()
        "#,
    )
    .bind(doc.run_id)
    .bind(&doc.external_id)
    .bind(doc.snapshot_id)
    .bind(&doc.content_hash)
    .bind(&doc.status)
    .bind(&doc.error)
    .bind(doc.byte_offset)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn corpus_dump_document_status(
    pool: &PgPool,
    run_id: Uuid,
    external_id: &str,
) -> anyhow::Result<Option<String>> {
    let status: Option<String> = sqlx::query_scalar(
        r#"
        SELECT status FROM corpus_dump_documents
        WHERE run_id = $1 AND external_id = $2
        "#,
    )
    .bind(run_id)
    .bind(external_id)
    .fetch_optional(pool)
    .await?;
    Ok(status)
}

pub async fn corpus_dump_document_status_counts(
    pool: &PgPool,
    run_id: Uuid,
) -> anyhow::Result<Vec<(String, i64)>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT status, COUNT(*)::bigint
        FROM corpus_dump_documents
        WHERE run_id = $1
        GROUP BY status
        ORDER BY status
        "#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
