// crates/talaria-store/src/phrase_candidates.rs
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PhraseCandidateRecord {
    pub sentence_id: Uuid,
    pub entity_id: Option<Uuid>,
    pub person_surface: String,
    pub time_surface: Option<String>,
    pub place_surface: Option<String>,
    pub verb_pivot: Option<String>,
    pub combinator_hash: String,
    pub extractor: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PendingCandidateRow {
    pub id: Uuid,
    pub sentence_id: Uuid,
    pub entity_id: Option<Uuid>,
    pub person_surface: String,
    pub time_surface: Option<String>,
    pub place_surface: Option<String>,
    pub verb_pivot: Option<String>,
    pub sentence_text: String,
}

pub async fn insert_phrase_candidate(
    pool: &PgPool,
    record: &PhraseCandidateRecord,
) -> anyhow::Result<Option<Uuid>> {
    let id: Option<Uuid> = sqlx::query_scalar(
        r#"
        INSERT INTO phrase_candidates (
            sentence_id, entity_id, person_surface, time_surface, place_surface,
            verb_pivot, combinator_hash, extractor, status
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending')
        ON CONFLICT (combinator_hash) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(record.sentence_id)
    .bind(record.entity_id)
    .bind(&record.person_surface)
    .bind(&record.time_surface)
    .bind(&record.place_surface)
    .bind(&record.verb_pivot)
    .bind(&record.combinator_hash)
    .bind(&record.extractor)
    .fetch_optional(pool)
    .await?;

    Ok(id)
}

pub async fn list_pending_candidates(
    pool: &PgPool,
    limit: i64,
) -> anyhow::Result<Vec<PendingCandidateRow>> {
    let rows = sqlx::query_as::<_, PendingCandidateRow>(
        r#"
        SELECT
            pc.id,
            pc.sentence_id,
            pc.entity_id,
            pc.person_surface,
            pc.time_surface,
            pc.place_surface,
            pc.verb_pivot,
            s.text AS sentence_text
        FROM phrase_candidates pc
        INNER JOIN sentences s ON s.id = pc.sentence_id
        WHERE pc.status = 'pending'
        ORDER BY pc.created_at ASC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn update_candidate_status(
    pool: &PgPool,
    candidate_id: Uuid,
    status: &str,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE phrase_candidates SET status = $2 WHERE id = $1")
        .bind(candidate_id)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(())
}
