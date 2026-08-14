// crates/talaria-store/src/cosmos_judgments.rs
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CosmosJudgmentInsert {
    pub fragment_id: Uuid,
    pub analyzer_id: String,
    pub version: String,
    pub score: f32,
    pub accepted: bool,
    pub signals: serde_json::Value,
    pub tuples: serde_json::Value,
    pub reject_reason: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CosmosJudgmentRow {
    pub id: Uuid,
    pub fragment_id: Uuid,
    pub analyzer_id: String,
    pub version: String,
    pub score: f32,
    pub accepted: bool,
    pub signals: serde_json::Value,
    pub tuples: serde_json::Value,
    pub reject_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CosmosJudgmentWrite {
    Inserted,
    Existing,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FragmentForCosmos {
    pub id: Uuid,
    pub snapshot_id: Uuid,
    pub text: String,
    pub start_offset: i32,
    pub title: Option<String>,
}

pub async fn insert_fragment_cosmos_judgment(
    pool: &PgPool,
    row: &CosmosJudgmentInsert,
) -> anyhow::Result<(Uuid, CosmosJudgmentWrite)> {
    let inserted: Option<(Uuid,)> = sqlx::query_as(
        r#"
        INSERT INTO fragment_cosmos_judgments (
            fragment_id, analyzer_id, version, score, accepted, signals, tuples, reject_reason
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
        ON CONFLICT (fragment_id, analyzer_id, version) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(row.fragment_id)
    .bind(&row.analyzer_id)
    .bind(&row.version)
    .bind(row.score)
    .bind(row.accepted)
    .bind(&row.signals)
    .bind(&row.tuples)
    .bind(&row.reject_reason)
    .fetch_optional(pool)
    .await?;
    if let Some((id,)) = inserted {
        return Ok((id, CosmosJudgmentWrite::Inserted));
    }
    let id: Uuid = sqlx::query_scalar(
        r#"
        SELECT id FROM fragment_cosmos_judgments
        WHERE fragment_id = $1 AND analyzer_id = $2 AND version = $3
        "#,
    )
    .bind(row.fragment_id)
    .bind(&row.analyzer_id)
    .bind(&row.version)
    .fetch_one(pool)
    .await?;
    Ok((id, CosmosJudgmentWrite::Existing))
}

pub async fn get_fragment_cosmos_judgment(
    pool: &PgPool,
    fragment_id: Uuid,
    analyzer_id: &str,
    version: &str,
) -> anyhow::Result<Option<CosmosJudgmentRow>> {
    let row = sqlx::query_as::<_, CosmosJudgmentRow>(
        r#"
        SELECT id, fragment_id, analyzer_id, version, score, accepted, signals, tuples, reject_reason
        FROM fragment_cosmos_judgments
        WHERE fragment_id = $1 AND analyzer_id = $2 AND version = $3
        "#,
    )
    .bind(fragment_id)
    .bind(analyzer_id)
    .bind(version)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn count_fragment_cosmos_judgments(
    pool: &PgPool,
    analyzer_id: &str,
    version: &str,
) -> anyhow::Result<i64> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint FROM fragment_cosmos_judgments
        WHERE analyzer_id = $1 AND version = $2
        "#,
    )
    .bind(analyzer_id)
    .bind(version)
    .fetch_one(pool)
    .await?;
    Ok(n)
}

pub async fn list_cosmos_accepted_fragments(
    pool: &PgPool,
    run_id: Option<Uuid>,
    source_kind: Option<&str>,
    analyzer_id: &str,
    version: &str,
    limit: i64,
) -> anyhow::Result<Vec<FragmentForCosmos>> {
    let cap = if limit > 0 { limit } else { i64::MAX };
    let rows = sqlx::query_as::<_, FragmentForCosmos>(
        r#"
        SELECT f.id, f.snapshot_id, f.text, f.start_offset, s.title
        FROM document_fragments f
        JOIN document_snapshots s ON s.id = f.snapshot_id
        JOIN fragment_cosmos_judgments j ON j.fragment_id = f.id
        WHERE f.fragment_kind = 'sentence'
          AND j.accepted
          AND j.analyzer_id = $1
          AND j.version = $2
          AND ($3::uuid IS NULL OR EXISTS (
                SELECT 1 FROM corpus_dump_documents d
                WHERE d.snapshot_id = f.snapshot_id AND d.run_id = $3
              ))
          AND ($4::text IS NULL OR s.source_type = $4)
        ORDER BY f.created_at ASC
        LIMIT $5
        "#,
    )
    .bind(analyzer_id)
    .bind(version)
    .bind(run_id)
    .bind(source_kind)
    .bind(cap)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_sentence_fragments_for_cosmos(
    pool: &PgPool,
    run_id: Option<Uuid>,
    source_kind: Option<&str>,
    analyzer_id: &str,
    version: &str,
    skip_existing: bool,
    limit: i64,
) -> anyhow::Result<Vec<FragmentForCosmos>> {
    let cap = if limit > 0 { limit } else { i64::MAX };
    let rows = sqlx::query_as::<_, FragmentForCosmos>(
        r#"
        SELECT f.id, f.snapshot_id, f.text, f.start_offset, s.title
        FROM document_fragments f
        JOIN document_snapshots s ON s.id = f.snapshot_id
        WHERE f.fragment_kind = 'sentence'
          AND ($1::uuid IS NULL OR EXISTS (
                SELECT 1 FROM corpus_dump_documents d
                WHERE d.snapshot_id = f.snapshot_id AND d.run_id = $1
              ))
          AND ($2::text IS NULL OR s.source_type = $2)
          AND (
                NOT $3::bool
                OR NOT EXISTS (
                    SELECT 1 FROM fragment_cosmos_judgments j
                    WHERE j.fragment_id = f.id
                      AND j.analyzer_id = $4
                      AND j.version = $5
                )
              )
        ORDER BY f.created_at ASC
        LIMIT $6
        "#,
    )
    .bind(run_id)
    .bind(source_kind)
    .bind(skip_existing)
    .bind(analyzer_id)
    .bind(version)
    .bind(cap)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
