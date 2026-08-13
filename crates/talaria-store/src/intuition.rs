// crates/talaria-store/src/intuition.rs
//! Intuition publication queue + debate source rows.

use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct QualityConflictRow {
    pub id: Uuid,
    pub occurrence_stem: Option<String>,
    pub event_type: String,
    pub predicate: String,
    pub place_label: Option<String>,
    pub time_json: serde_json::Value,
    pub canonical_event_id: Option<Uuid>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SoftClaimExportRow {
    pub id: Uuid,
    pub claim_kind: String,
    pub text: String,
    pub place_label: Option<String>,
    pub canonical_event_id: Option<Uuid>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EventPointerRow {
    pub id: Uuid,
    pub title: String,
    pub place_label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IntuitionPublicationInsert {
    pub subject_entity_id: Uuid,
    pub debate_id: String,
    pub bundle_fingerprint: String,
    pub kind: String,
    pub status: String,
    pub payload_json: serde_json::Value,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct IntuitionPublicationRow {
    pub id: Uuid,
    pub debate_id: String,
    pub bundle_fingerprint: String,
    pub kind: String,
    pub status: String,
    pub triple_term_id: Option<String>,
    pub tx_hash: Option<String>,
}

pub async fn list_conflict_quality_claims(
    pool: &PgPool,
    subject_entity_id: Uuid,
) -> anyhow::Result<Vec<QualityConflictRow>> {
    let rows = sqlx::query_as::<_, QualityConflictRow>(
        r#"
        SELECT id, occurrence_stem, event_type, predicate, place_label, time_json, canonical_event_id
        FROM quality_claims
        WHERE subject_entity_id = $1 AND status = 'conflict'
        ORDER BY occurrence_stem NULLS LAST, created_at ASC
        "#,
    )
    .bind(subject_entity_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_exportable_soft_claims(
    pool: &PgPool,
    entity_id: Uuid,
) -> anyhow::Result<Vec<SoftClaimExportRow>> {
    let rows = sqlx::query_as::<_, SoftClaimExportRow>(
        r#"
        SELECT id, claim_kind, text, place_label, canonical_event_id
        FROM soft_claims
        WHERE entity_id = $1
          AND claim_kind IN ('theory', 'controversy', 'debate_stance')
        ORDER BY created_at ASC
        "#,
    )
    .bind(entity_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_quality_event_pointer(
    pool: &PgPool,
    event_id: Uuid,
) -> anyhow::Result<Option<EventPointerRow>> {
    let row = sqlx::query_as::<_, EventPointerRow>(
        r#"
        SELECT id, title, place_label
        FROM canonical_events
        WHERE id = $1 AND pipeline = 'quality'
        "#,
    )
    .bind(event_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn find_quality_event_for_stem(
    pool: &PgPool,
    subject_entity_id: Uuid,
    stem: &str,
) -> anyhow::Result<Option<EventPointerRow>> {
    let row = sqlx::query_as::<_, EventPointerRow>(
        r#"
        SELECT id, title, place_label
        FROM canonical_events
        WHERE entity_id = $1
          AND occurrence_stem = $2
          AND pipeline = 'quality'
          AND is_active
        ORDER BY created_at ASC
        LIMIT 1
        "#,
    )
    .bind(subject_entity_id)
    .bind(stem)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn upsert_intuition_publication(
    pool: &PgPool,
    row: &IntuitionPublicationInsert,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO intuition_publications (
            subject_entity_id, debate_id, bundle_fingerprint, kind, status, payload_json
        )
        VALUES ($1,$2,$3,$4,$5,$6)
        ON CONFLICT (bundle_fingerprint) DO UPDATE SET
            payload_json = EXCLUDED.payload_json,
            kind = EXCLUDED.kind,
            debate_id = EXCLUDED.debate_id,
            status = CASE
                WHEN intuition_publications.status = 'published' THEN intuition_publications.status
                ELSE EXCLUDED.status
            END,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(row.subject_entity_id)
    .bind(&row.debate_id)
    .bind(&row.bundle_fingerprint)
    .bind(&row.kind)
    .bind(&row.status)
    .bind(&row.payload_json)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn get_intuition_publication_by_fingerprint(
    pool: &PgPool,
    fingerprint: &str,
) -> anyhow::Result<Option<IntuitionPublicationRow>> {
    let row = sqlx::query_as::<_, IntuitionPublicationRow>(
        r#"
        SELECT id, debate_id, bundle_fingerprint, kind, status, triple_term_id, tx_hash
        FROM intuition_publications
        WHERE bundle_fingerprint = $1
        "#,
    )
    .bind(fingerprint)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn mark_intuition_published(
    pool: &PgPool,
    id: Uuid,
    chain_id: i32,
    question_term_id: Option<&str>,
    triple_term_id: Option<&str>,
    tx_hash: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE intuition_publications SET
            status = 'published',
            chain_id = $2,
            question_term_id = $3,
            triple_term_id = $4,
            tx_hash = $5,
            last_error = NULL,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(chain_id)
    .bind(question_term_id)
    .bind(triple_term_id)
    .bind(tx_hash)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_intuition_failed(pool: &PgPool, id: Uuid, err: &str) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE intuition_publications SET
            status = 'failed',
            last_error = $2,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(err)
    .execute(pool)
    .await?;
    Ok(())
}
