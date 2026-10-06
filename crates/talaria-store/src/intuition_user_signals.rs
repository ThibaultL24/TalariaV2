// crates/talaria-store/src/intuition_user_signals.rs
//! Immutable Intuition deposit records. Not the current Talaria stance.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct IntuitionUserSignalRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub claim_id: Uuid,
    pub interaction_id: Option<Uuid>,
    pub action_type: String,
    pub chain_id: i64,
    pub term_id: String,
    pub curve_id: String,
    pub tx_hash: String,
    pub assets: String,
    pub shares: Option<String>,
    pub receiver: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct IntuitionUserSignalInsert {
    pub user_id: Uuid,
    pub claim_id: Uuid,
    pub interaction_id: Option<Uuid>,
    pub action_type: String,
    pub chain_id: i64,
    pub term_id: String,
    pub curve_id: String,
    pub tx_hash: String,
    pub assets: String,
    pub shares: Option<String>,
    pub receiver: String,
}

const SELECT_COLS: &str = r#"
    id, user_id, claim_id, interaction_id, action_type, chain_id, term_id,
    curve_id::text AS curve_id, tx_hash, assets::text AS assets, shares::text AS shares,
    receiver, status, created_at, confirmed_at
"#;

pub async fn get_signal_by_tx_hash(
    pool: &PgPool,
    tx_hash: &str,
) -> anyhow::Result<Option<IntuitionUserSignalRow>> {
    let row = sqlx::query_as::<_, IntuitionUserSignalRow>(&format!(
        "SELECT {SELECT_COLS} FROM intuition_user_signals WHERE tx_hash = $1"
    ))
    .bind(tx_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn insert_confirmed_signal(
    pool: &PgPool,
    row: &IntuitionUserSignalInsert,
) -> anyhow::Result<IntuitionUserSignalRow> {
    let inserted = sqlx::query_as::<_, IntuitionUserSignalRow>(&format!(
        r#"
        INSERT INTO intuition_user_signals (
            user_id, claim_id, interaction_id, action_type, chain_id, term_id,
            curve_id, tx_hash, assets, shares, receiver, status, confirmed_at
        )
        VALUES (
            $1,$2,$3,$4,$5,$6,$7::numeric,$8,$9::numeric,$10::numeric,$11,'confirmed', NOW()
        )
        ON CONFLICT (tx_hash) DO NOTHING
        RETURNING {SELECT_COLS}
        "#
    ))
    .bind(row.user_id)
    .bind(row.claim_id)
    .bind(row.interaction_id)
    .bind(&row.action_type)
    .bind(row.chain_id)
    .bind(&row.term_id)
    .bind(&row.curve_id)
    .bind(&row.tx_hash)
    .bind(&row.assets)
    .bind(&row.shares)
    .bind(&row.receiver)
    .fetch_optional(pool)
    .await?;
    if let Some(row) = inserted {
        return Ok(row);
    }
    get_signal_by_tx_hash(pool, &row.tx_hash)
        .await?
        .ok_or_else(|| anyhow::anyhow!("signal missing after unique conflict"))
}

#[derive(Debug, Clone)]
pub struct ClaimIntuitionBinding {
    pub publication_id: Uuid,
    pub chain_id: i32,
    pub triple_term_id: String,
}

pub async fn get_published_claim_intuition(
    pool: &PgPool,
    claim_id: Uuid,
) -> anyhow::Result<Option<ClaimIntuitionBinding>> {
    let row = sqlx::query_as::<_, (Uuid, i32, String)>(
        r#"
        SELECT p.id, COALESCE(p.chain_id, 13579), p.triple_term_id
        FROM intuition_publications p
        WHERE p.soft_claim_id = $1
          AND p.status = 'published'
          AND p.triple_term_id IS NOT NULL
          AND length(trim(p.triple_term_id)) > 0
        ORDER BY p.updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(claim_id)
    .fetch_optional(pool)
    .await?;
    if let Some((publication_id, chain_id, triple_term_id)) = row {
        return Ok(Some(ClaimIntuitionBinding {
            publication_id,
            chain_id,
            triple_term_id,
        }));
    }
    let fallback = sqlx::query_as::<_, (Uuid, i32, String)>(
        r#"
        SELECT p.id, COALESCE(p.chain_id, 13579), b.term_id
        FROM intuition_publications p
        JOIN intuition_term_bindings b ON b.publication_id = p.id
        WHERE p.soft_claim_id = $1
          AND p.status = 'published'
          AND b.role = 'question_has_proposition'
        ORDER BY p.updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(claim_id)
    .fetch_optional(pool)
    .await?;
    Ok(fallback.map(|(publication_id, chain_id, triple_term_id)| {
        ClaimIntuitionBinding {
            publication_id,
            chain_id,
            triple_term_id,
        }
    }))
}
