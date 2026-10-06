// crates/talaria-store/src/comments.rs
//! Agora comments and semantic reactions. Claim-scoped, off-chain.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

pub const MAX_COMMENT_CHARS: usize = 4000;
pub const DEFAULT_ROOT_LIMIT: i64 = 20;
pub const MAX_ROOT_LIMIT: i64 = 50;
pub const MAX_REPLIES_PER_ROOT: i64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentReactionType {
    Relevant,
    WellSourced,
    Interesting,
    NeedsNuance,
    Disagree,
}

impl CommentReactionType {
    pub const ALL: [Self; 5] = [
        Self::Relevant,
        Self::WellSourced,
        Self::Interesting,
        Self::NeedsNuance,
        Self::Disagree,
    ];

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "relevant" => Some(Self::Relevant),
            "well_sourced" => Some(Self::WellSourced),
            "interesting" => Some(Self::Interesting),
            "needs_nuance" => Some(Self::NeedsNuance),
            "disagree" => Some(Self::Disagree),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Relevant => "relevant",
            Self::WellSourced => "well_sourced",
            Self::Interesting => "interesting",
            Self::NeedsNuance => "needs_nuance",
            Self::Disagree => "disagree",
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommentRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub claim_id: Uuid,
    pub parent_comment_id: Option<Uuid>,
    pub body: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReactionCountRow {
    pub comment_id: Uuid,
    pub reaction_type: String,
    pub n: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReactionMineRow {
    pub comment_id: Uuid,
    pub reaction_type: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommentCountRow {
    pub claim_id: Uuid,
    pub n: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReplyCountRow {
    pub parent_comment_id: Uuid,
    pub n: i64,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct CommentAuthorRow {
    pub id: Uuid,
    pub display_name: Option<String>,
}

const COMMENT_COLS: &str = r#"
    id, user_id, claim_id, parent_comment_id, body, status,
    created_at, updated_at, edited_at
"#;

pub fn normalize_comment_body(raw: &str) -> Result<String, ()> {
    let body = raw.trim().to_string();
    let n = body.chars().count();
    if n < 1 || n > MAX_COMMENT_CHARS {
        return Err(());
    }
    Ok(body)
}

pub async fn insert_comment(
    pool: &PgPool,
    user_id: Uuid,
    claim_id: Uuid,
    parent_comment_id: Option<Uuid>,
    body: &str,
) -> anyhow::Result<CommentRow> {
    let row = sqlx::query_as::<_, CommentRow>(&format!(
        r#"
        INSERT INTO comments (user_id, claim_id, parent_comment_id, body)
        VALUES ($1, $2, $3, $4)
        RETURNING {COMMENT_COLS}
        "#
    ))
    .bind(user_id)
    .bind(claim_id)
    .bind(parent_comment_id)
    .bind(body)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_comment(pool: &PgPool, id: Uuid) -> anyhow::Result<Option<CommentRow>> {
    let row = sqlx::query_as::<_, CommentRow>(&format!(
        "SELECT {COMMENT_COLS} FROM comments WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn update_owned_comment_body(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
    body: &str,
) -> anyhow::Result<Option<CommentRow>> {
    let row = sqlx::query_as::<_, CommentRow>(&format!(
        r#"
        UPDATE comments
        SET body = $3, updated_at = NOW(), edited_at = NOW()
        WHERE id = $1 AND user_id = $2 AND status = 'active'
        RETURNING {COMMENT_COLS}
        "#
    ))
    .bind(id)
    .bind(user_id)
    .bind(body)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn soft_delete_owned_comment(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> anyhow::Result<Option<CommentRow>> {
    let row = sqlx::query_as::<_, CommentRow>(&format!(
        r#"
        UPDATE comments
        SET status = 'deleted', updated_at = NOW()
        WHERE id = $1 AND user_id = $2 AND status = 'active'
        RETURNING {COMMENT_COLS}
        "#
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn list_root_comments(
    pool: &PgPool,
    claim_id: Uuid,
    cursor_created_at: Option<DateTime<Utc>>,
    cursor_id: Option<Uuid>,
    limit: i64,
) -> anyhow::Result<Vec<CommentRow>> {
    let rows = sqlx::query_as::<_, CommentRow>(&format!(
        r#"
        SELECT {COMMENT_COLS}
        FROM comments
        WHERE claim_id = $1
          AND parent_comment_id IS NULL
          AND (
            $2::timestamptz IS NULL
            OR created_at < $2
            OR (created_at = $2 AND id < $3)
          )
        ORDER BY created_at DESC, id DESC
        LIMIT $4
        "#
    ))
    .bind(claim_id)
    .bind(cursor_created_at)
    .bind(cursor_id.unwrap_or(Uuid::nil()))
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn count_replies_for_parents(
    pool: &PgPool,
    parent_ids: &[Uuid],
) -> anyhow::Result<Vec<ReplyCountRow>> {
    if parent_ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query_as::<_, ReplyCountRow>(
        r#"
        SELECT parent_comment_id, COUNT(*)::bigint AS n
        FROM comments
        WHERE parent_comment_id = ANY($1)
        GROUP BY parent_comment_id
        "#,
    )
    .bind(parent_ids)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_replies_for_parents(
    pool: &PgPool,
    parent_ids: &[Uuid],
    per_parent: i64,
) -> anyhow::Result<Vec<CommentRow>> {
    if parent_ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query_as::<_, CommentRow>(&format!(
        r#"
        SELECT {COMMENT_COLS}
        FROM (
            SELECT {COMMENT_COLS},
                   row_number() OVER (
                       PARTITION BY parent_comment_id
                       ORDER BY created_at ASC, id ASC
                   ) AS rn
            FROM comments
            WHERE parent_comment_id = ANY($1)
        ) ranked
        WHERE rn <= $2
        ORDER BY parent_comment_id, created_at ASC, id ASC
        "#
    ))
    .bind(parent_ids)
    .bind(per_parent)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn aggregate_reactions(
    pool: &PgPool,
    comment_ids: &[Uuid],
) -> anyhow::Result<Vec<ReactionCountRow>> {
    if comment_ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query_as::<_, ReactionCountRow>(
        r#"
        SELECT comment_id, reaction_type, COUNT(*)::bigint AS n
        FROM comment_reactions
        WHERE comment_id = ANY($1)
        GROUP BY comment_id, reaction_type
        "#,
    )
    .bind(comment_ids)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_my_reactions(
    pool: &PgPool,
    user_id: Uuid,
    comment_ids: &[Uuid],
) -> anyhow::Result<Vec<ReactionMineRow>> {
    if comment_ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query_as::<_, ReactionMineRow>(
        r#"
        SELECT comment_id, reaction_type
        FROM comment_reactions
        WHERE user_id = $1 AND comment_id = ANY($2)
        "#,
    )
    .bind(user_id)
    .bind(comment_ids)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_comment_authors(
    pool: &PgPool,
    user_ids: &[Uuid],
) -> anyhow::Result<Vec<CommentAuthorRow>> {
    if user_ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query_as::<_, CommentAuthorRow>(
        "SELECT id, display_name FROM users WHERE id = ANY($1)",
    )
    .bind(user_ids)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn count_active_comments_for_claims(
    pool: &PgPool,
    claim_ids: &[Uuid],
) -> anyhow::Result<Vec<CommentCountRow>> {
    if claim_ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query_as::<_, CommentCountRow>(
        r#"
        SELECT claim_id, COUNT(*)::bigint AS n
        FROM comments
        WHERE claim_id = ANY($1) AND status = 'active'
        GROUP BY claim_id
        "#,
    )
    .bind(claim_ids)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn insert_reaction(
    pool: &PgPool,
    comment_id: Uuid,
    user_id: Uuid,
    reaction_type: CommentReactionType,
) -> anyhow::Result<bool> {
    let result = sqlx::query(
        r#"
        INSERT INTO comment_reactions (comment_id, user_id, reaction_type)
        VALUES ($1, $2, $3)
        ON CONFLICT (comment_id, user_id, reaction_type) DO NOTHING
        "#,
    )
    .bind(comment_id)
    .bind(user_id)
    .bind(reaction_type.as_str())
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete_reaction(
    pool: &PgPool,
    comment_id: Uuid,
    user_id: Uuid,
    reaction_type: CommentReactionType,
) -> anyhow::Result<bool> {
    let result = sqlx::query(
        r#"
        DELETE FROM comment_reactions
        WHERE comment_id = $1 AND user_id = $2 AND reaction_type = $3
        "#,
    )
    .bind(comment_id)
    .bind(user_id)
    .bind(reaction_type.as_str())
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}
