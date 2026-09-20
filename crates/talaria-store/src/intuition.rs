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
    pub event_type: String,
    pub time_json: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct IntuitionPublicationInsert {
    pub subject_entity_id: Uuid,
    pub debate_id: String,
    pub bundle_fingerprint: String,
    pub kind: String,
    pub status: String,
    pub payload_json: serde_json::Value,
    pub soft_claim_id: Option<Uuid>,
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
    pub payload_json: Option<serde_json::Value>,
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

pub async fn get_person_event_pointer(
    pool: &PgPool,
    event_id: Uuid,
) -> anyhow::Result<Option<EventPointerRow>> {
    let row = sqlx::query_as::<_, EventPointerRow>(
        r#"
        SELECT id, title, place_label, event_type, time_json
        FROM canonical_events
        WHERE id = $1 AND pipeline = 'person' AND is_active
        "#,
    )
    .bind(event_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Legacy alias — same as [`get_person_event_pointer`].
pub async fn get_quality_event_pointer(
    pool: &PgPool,
    event_id: Uuid,
) -> anyhow::Result<Option<EventPointerRow>> {
    get_person_event_pointer(pool, event_id).await
}

pub async fn find_person_event_for_stem(
    pool: &PgPool,
    subject_entity_id: Uuid,
    stem: &str,
) -> anyhow::Result<Option<EventPointerRow>> {
    let row = sqlx::query_as::<_, EventPointerRow>(
        r#"
        SELECT id, title, place_label, event_type, time_json
        FROM canonical_events
        WHERE entity_id = $1
          AND occurrence_stem = $2
          AND pipeline = 'person'
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

/// Legacy alias — same as [`find_person_event_for_stem`].
pub async fn find_quality_event_for_stem(
    pool: &PgPool,
    subject_entity_id: Uuid,
    stem: &str,
) -> anyhow::Result<Option<EventPointerRow>> {
    find_person_event_for_stem(pool, subject_entity_id, stem).await
}

pub async fn find_intuition_publication_for_claim(
    pool: &PgPool,
    claim_id: Uuid,
) -> anyhow::Result<Option<IntuitionPublicationRow>> {
    let row = sqlx::query_as::<_, IntuitionPublicationRow>(
        r#"
        SELECT id, debate_id, bundle_fingerprint, kind, status, triple_term_id, tx_hash, payload_json
        FROM intuition_publications
        WHERE soft_claim_id = $1
        ORDER BY CASE WHEN status = 'published' THEN 0 ELSE 1 END, updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(claim_id)
    .fetch_optional(pool)
    .await?;
    if row.is_some() {
        return Ok(row);
    }
    // Legacy rows (pre-030): fall back to debate_id / payload scan.
    let needle = format!("%{claim_id}%");
    let row = sqlx::query_as::<_, IntuitionPublicationRow>(
        r#"
        SELECT id, debate_id, bundle_fingerprint, kind, status, triple_term_id, tx_hash, payload_json
        FROM intuition_publications
        WHERE kind IN ('theory', 'controversy', 'debate_stance')
          AND (
            debate_id ILIKE $1
            OR COALESCE(payload_json::text, '') ILIKE $1
          )
        ORDER BY CASE WHEN status = 'published' THEN 0 ELSE 1 END, updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(needle)
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
            subject_entity_id, debate_id, bundle_fingerprint, kind, status, payload_json, soft_claim_id
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7)
        ON CONFLICT (bundle_fingerprint) DO UPDATE SET
            payload_json = CASE
                WHEN intuition_publications.status = 'published' THEN intuition_publications.payload_json
                ELSE EXCLUDED.payload_json
            END,
            kind = CASE
                WHEN intuition_publications.status = 'published' THEN intuition_publications.kind
                ELSE EXCLUDED.kind
            END,
            debate_id = CASE
                WHEN intuition_publications.status = 'published' THEN intuition_publications.debate_id
                ELSE EXCLUDED.debate_id
            END,
            soft_claim_id = CASE
                WHEN intuition_publications.status = 'published' THEN intuition_publications.soft_claim_id
                ELSE COALESCE(EXCLUDED.soft_claim_id, intuition_publications.soft_claim_id)
            END,
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
    .bind(row.soft_claim_id)
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
        SELECT id, debate_id, bundle_fingerprint, kind, status, triple_term_id, tx_hash, payload_json
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
        WHERE id = $1 AND status <> 'published' AND $4 IS NOT NULL
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

pub async fn mark_intuition_retryable(
    pool: &PgPool,
    id: Uuid,
    status: &str,
    err: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE intuition_publications SET
            status = $3,
            last_error = $2,
            updated_at = NOW()
        WHERE id = $1 AND status <> 'published'
        "#,
    )
    .bind(id)
    .bind(err)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct IntuitionTermBindingInsert {
    pub publication_id: Uuid,
    pub role: String,
    pub local_kind: String,
    pub local_id: Option<String>,
    pub chain_id: i32,
    pub term_id: String,
    pub ipfs_uri: Option<String>,
    pub data_hash: Option<String>,
    pub classification: Option<String>,
    pub created_on_chain: bool,
    pub verified: bool,
}

pub async fn upsert_intuition_term_binding(
    pool: &PgPool,
    row: &IntuitionTermBindingInsert,
) -> anyhow::Result<()> {
    let verified_at = if row.verified {
        Some(chrono::Utc::now())
    } else {
        None
    };
    sqlx::query(
        r#"
        INSERT INTO intuition_term_bindings (
            publication_id, role, local_kind, local_id, chain_id, term_id,
            ipfs_uri, data_hash, classification, created_on_chain, verified_at
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
        ON CONFLICT (publication_id, role) DO UPDATE SET
            term_id = EXCLUDED.term_id,
            ipfs_uri = COALESCE(EXCLUDED.ipfs_uri, intuition_term_bindings.ipfs_uri),
            data_hash = COALESCE(EXCLUDED.data_hash, intuition_term_bindings.data_hash),
            classification = COALESCE(EXCLUDED.classification, intuition_term_bindings.classification),
            created_on_chain = intuition_term_bindings.created_on_chain OR EXCLUDED.created_on_chain,
            verified_at = COALESCE(EXCLUDED.verified_at, intuition_term_bindings.verified_at)
        "#,
    )
    .bind(row.publication_id)
    .bind(&row.role)
    .bind(&row.local_kind)
    .bind(&row.local_id)
    .bind(row.chain_id)
    .bind(&row.term_id)
    .bind(&row.ipfs_uri)
    .bind(&row.data_hash)
    .bind(&row.classification)
    .bind(row.created_on_chain)
    .bind(verified_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_intuition_failed(pool: &PgPool, id: Uuid, err: &str) -> anyhow::Result<()> {
    mark_intuition_retryable(pool, id, "failed", err).await
}

pub async fn mark_intuition_pin_failed(pool: &PgPool, id: Uuid, err: &str) -> anyhow::Result<()> {
    mark_intuition_retryable(pool, id, "pin_failed", err).await
}
