// crates/talaria-api/src/comments.rs
//! Agora comments: conversational, off-chain, never mapped to Intuition.

use crate::agora_stance::is_stance_claim;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;
use talaria_store::{
    aggregate_reactions, count_replies_for_parents, delete_reaction, get_claim, get_comment,
    insert_comment, insert_reaction, list_comment_authors, list_my_reactions,
    list_replies_for_parents, list_root_comments, normalize_comment_body, soft_delete_owned_comment,
    update_owned_comment_body, CommentReactionType, CommentRow, DEFAULT_ROOT_LIMIT,
    MAX_REPLIES_PER_ROOT, MAX_ROOT_LIMIT,
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentError {
    Unauthenticated,
    ClaimNotFound,
    ClaimNotDiscussable,
    CommentNotFound,
    InvalidCommentBody,
    InvalidParentComment,
    InvalidReaction,
    DatabaseError,
}

impl CommentError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthenticated => "unauthenticated",
            Self::ClaimNotFound => "claim_not_found",
            Self::ClaimNotDiscussable => "claim_not_discussable",
            Self::CommentNotFound => "comment_not_found",
            Self::InvalidCommentBody => "invalid_comment_body",
            Self::InvalidParentComment => "invalid_parent_comment",
            Self::InvalidReaction => "invalid_reaction",
            Self::DatabaseError => "database_error",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::Unauthenticated => "Not authenticated",
            Self::ClaimNotFound => "Claim not found",
            Self::ClaimNotDiscussable => "Comments are only allowed on Agora claims",
            Self::CommentNotFound => "Comment not found",
            Self::InvalidCommentBody => "Comment body must be 1–4000 characters after trim",
            Self::InvalidParentComment => "Replies are only allowed on a root comment of this claim",
            Self::InvalidReaction => "Unknown reaction type",
            Self::DatabaseError => "Internal database error",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CommentAuthorDto {
    pub id: Uuid,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommentDto {
    pub id: Uuid,
    pub claim_id: Uuid,
    pub author: CommentAuthorDto,
    pub body: Option<String>,
    pub status: String,
    pub edited_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub reactions: serde_json::Map<String, serde_json::Value>,
    pub my_reactions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replies: Option<Vec<CommentDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_count: Option<i64>,
}

fn empty_reactions() -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    for kind in CommentReactionType::ALL {
        map.insert(kind.as_str().to_string(), serde_json::Value::Number(0.into()));
    }
    map
}

fn map_db<E>(_: E) -> CommentError {
    CommentError::DatabaseError
}

async fn require_discussable_claim(pool: &PgPool, claim_id: Uuid) -> Result<(), CommentError> {
    let claim = get_claim(pool, claim_id)
        .await
        .map_err(map_db)?
        .ok_or(CommentError::ClaimNotFound)?;
    if !is_stance_claim(&claim.claim_kind) {
        return Err(CommentError::ClaimNotDiscussable);
    }
    Ok(())
}

fn encode_cursor(row: &CommentRow) -> String {
    format!("{}_{}", row.created_at.timestamp_micros(), row.id)
}

fn decode_cursor(cursor: Option<&str>) -> (Option<DateTime<Utc>>, Option<Uuid>) {
    let Some(raw) = cursor.map(str::trim).filter(|s| !s.is_empty()) else {
        return (None, None);
    };
    let mut parts = raw.splitn(2, '_');
    let ts = parts
        .next()
        .and_then(|s| s.parse::<i64>().ok())
        .and_then(DateTime::from_timestamp_micros);
    let id = parts.next().and_then(|s| Uuid::parse_str(s).ok());
    (ts, id)
}

fn clamp_limit(limit: Option<i64>) -> i64 {
    match limit {
        Some(n) if n < 1 => DEFAULT_ROOT_LIMIT,
        Some(n) if n > MAX_ROOT_LIMIT => MAX_ROOT_LIMIT,
        Some(n) => n,
        None => DEFAULT_ROOT_LIMIT,
    }
}

async fn assemble_dtos(
    pool: &PgPool,
    rows: &[CommentRow],
    viewer: Option<Uuid>,
) -> Result<Vec<CommentDto>, CommentError> {
    if rows.is_empty() {
        return Ok(vec![]);
    }
    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let author_ids: Vec<Uuid> = {
        let mut v: Vec<Uuid> = rows.iter().map(|r| r.user_id).collect();
        v.sort();
        v.dedup();
        v
    };
    let authors = list_comment_authors(pool, &author_ids)
        .await
        .map_err(map_db)?;
    let mut author_map: HashMap<Uuid, Option<String>> = HashMap::new();
    for a in authors {
        author_map.insert(a.id, a.display_name);
    }
    let counts = aggregate_reactions(pool, &ids).await.map_err(map_db)?;
    let mut count_map: HashMap<(Uuid, String), i64> = HashMap::new();
    for row in counts {
        count_map.insert((row.comment_id, row.reaction_type), row.n);
    }
    let mine = if let Some(uid) = viewer {
        list_my_reactions(pool, uid, &ids).await.map_err(map_db)?
    } else {
        vec![]
    };
    let mut mine_map: HashMap<Uuid, Vec<String>> = HashMap::new();
    for row in mine {
        mine_map.entry(row.comment_id).or_default().push(row.reaction_type);
    }
    Ok(rows
        .iter()
        .map(|row| {
            let mut reactions = empty_reactions();
            for kind in CommentReactionType::ALL {
                if let Some(n) = count_map.get(&(row.id, kind.as_str().to_string())) {
                    reactions.insert(kind.as_str().to_string(), serde_json::Value::Number((*n).into()));
                }
            }
            let deleted = row.status == "deleted";
            CommentDto {
                id: row.id,
                claim_id: row.claim_id,
                author: CommentAuthorDto {
                    id: row.user_id,
                    display_name: author_map.get(&row.user_id).cloned().flatten(),
                },
                body: if deleted {
                    None
                } else {
                    Some(row.body.clone())
                },
                status: row.status.clone(),
                edited_at: row.edited_at,
                created_at: row.created_at,
                reactions,
                my_reactions: mine_map.remove(&row.id).unwrap_or_default(),
                replies: None,
                reply_count: None,
            }
        })
        .collect())
}

pub async fn list_claim_comments(
    pool: &PgPool,
    claim_id: Uuid,
    viewer: Option<Uuid>,
    cursor: Option<&str>,
    limit: Option<i64>,
) -> Result<(Vec<CommentDto>, Option<String>), CommentError> {
    require_discussable_claim(pool, claim_id).await?;
    let limit = clamp_limit(limit);
    let (ts, id) = decode_cursor(cursor);
    let mut roots = list_root_comments(pool, claim_id, ts, id, limit + 1)
        .await
        .map_err(map_db)?;
    let next = if roots.len() as i64 > limit {
        roots.get(limit as usize - 1).map(encode_cursor)
    } else {
        None
    };
    roots.truncate(limit as usize);
    let parent_ids: Vec<Uuid> = roots.iter().map(|r| r.id).collect();
    let reply_counts = count_replies_for_parents(pool, &parent_ids)
        .await
        .map_err(map_db)?;
    let mut reply_count_map: HashMap<Uuid, i64> = HashMap::new();
    for row in reply_counts {
        reply_count_map.insert(row.parent_comment_id, row.n);
    }
    let replies = list_replies_for_parents(pool, &parent_ids, MAX_REPLIES_PER_ROOT)
        .await
        .map_err(map_db)?;
    let mut all_rows = roots.clone();
    all_rows.extend(replies.clone());
    let mut dtos = assemble_dtos(pool, &all_rows, viewer).await?;
    let mut dto_by_id: HashMap<Uuid, CommentDto> = dtos.drain(..).map(|d| (d.id, d)).collect();
    let mut replies_by_parent: HashMap<Uuid, Vec<CommentDto>> = HashMap::new();
    for reply in &replies {
        if let Some(dto) = dto_by_id.remove(&reply.id) {
            replies_by_parent
                .entry(reply.parent_comment_id.unwrap_or(reply.id))
                .or_default()
                .push(dto);
        }
    }
    let page = roots
        .into_iter()
        .filter_map(|root| {
            let mut dto = dto_by_id.remove(&root.id)?;
            dto.reply_count = Some(reply_count_map.get(&root.id).copied().unwrap_or(0));
            dto.replies = Some(replies_by_parent.remove(&root.id).unwrap_or_default());
            Some(dto)
        })
        .collect();
    Ok((page, next))
}

pub async fn create_comment(
    pool: &PgPool,
    user_id: Uuid,
    claim_id: Uuid,
    body: &str,
    parent_comment_id: Option<Uuid>,
) -> Result<CommentDto, CommentError> {
    require_discussable_claim(pool, claim_id).await?;
    let body = normalize_comment_body(body).map_err(|_| CommentError::InvalidCommentBody)?;
    if let Some(parent_id) = parent_comment_id {
        let parent = get_comment(pool, parent_id)
            .await
            .map_err(map_db)?
            .ok_or(CommentError::InvalidParentComment)?;
        if parent.claim_id != claim_id || parent.parent_comment_id.is_some() {
            return Err(CommentError::InvalidParentComment);
        }
    }
    let row = insert_comment(pool, user_id, claim_id, parent_comment_id, &body)
        .await
        .map_err(map_db)?;
    Ok(assemble_dtos(pool, &[row], Some(user_id))
        .await?
        .into_iter()
        .next()
        .ok_or(CommentError::DatabaseError)?)
}

pub async fn patch_comment(
    pool: &PgPool,
    user_id: Uuid,
    comment_id: Uuid,
    body: &str,
) -> Result<CommentDto, CommentError> {
    let body = normalize_comment_body(body).map_err(|_| CommentError::InvalidCommentBody)?;
    let row = update_owned_comment_body(pool, comment_id, user_id, &body)
        .await
        .map_err(map_db)?
        .ok_or(CommentError::CommentNotFound)?;
    Ok(assemble_dtos(pool, &[row], Some(user_id))
        .await?
        .into_iter()
        .next()
        .ok_or(CommentError::DatabaseError)?)
}

pub async fn delete_comment(
    pool: &PgPool,
    user_id: Uuid,
    comment_id: Uuid,
) -> Result<CommentDto, CommentError> {
    let row = soft_delete_owned_comment(pool, comment_id, user_id)
        .await
        .map_err(map_db)?
        .ok_or(CommentError::CommentNotFound)?;
    Ok(assemble_dtos(pool, &[row], Some(user_id))
        .await?
        .into_iter()
        .next()
        .ok_or(CommentError::DatabaseError)?)
}

pub async fn add_comment_reaction(
    pool: &PgPool,
    user_id: Uuid,
    comment_id: Uuid,
    reaction_raw: &str,
) -> Result<(CommentDto, bool), CommentError> {
    let kind = CommentReactionType::parse(reaction_raw).ok_or(CommentError::InvalidReaction)?;
    let comment = get_comment(pool, comment_id)
        .await
        .map_err(map_db)?
        .ok_or(CommentError::CommentNotFound)?;
    if comment.status != "active" {
        return Err(CommentError::CommentNotFound);
    }
    let created = insert_reaction(pool, comment_id, user_id, kind)
        .await
        .map_err(map_db)?;
    let refreshed = get_comment(pool, comment_id)
        .await
        .map_err(map_db)?
        .ok_or(CommentError::CommentNotFound)?;
    let dto = assemble_dtos(pool, &[refreshed], Some(user_id))
        .await?
        .into_iter()
        .next()
        .ok_or(CommentError::DatabaseError)?;
    Ok((dto, created))
}

pub async fn remove_comment_reaction(
    pool: &PgPool,
    user_id: Uuid,
    comment_id: Uuid,
    reaction_raw: &str,
) -> Result<CommentDto, CommentError> {
    let kind = CommentReactionType::parse(reaction_raw).ok_or(CommentError::InvalidReaction)?;
    let comment = get_comment(pool, comment_id)
        .await
        .map_err(map_db)?
        .ok_or(CommentError::CommentNotFound)?;
    if comment.status != "active" {
        return Err(CommentError::CommentNotFound);
    }
    delete_reaction(pool, comment_id, user_id, kind)
        .await
        .map_err(map_db)?;
    let refreshed = get_comment(pool, comment_id)
        .await
        .map_err(map_db)?
        .ok_or(CommentError::CommentNotFound)?;
    Ok(assemble_dtos(pool, &[refreshed], Some(user_id))
        .await?
        .into_iter()
        .next()
        .ok_or(CommentError::DatabaseError)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use talaria_store::{
        insert_claim, resolve_or_create_evm_user, upsert_entity_with_kind, ClaimInsert,
        NormalizedEvmAddress,
    };

    async fn user(pool: &PgPool, hex: &str) -> talaria_store::UserRow {
        resolve_or_create_evm_user(pool, &NormalizedEvmAddress::parse(hex).unwrap())
            .await
            .unwrap()
    }

    async fn claim(pool: &PgPool, kind: &str) -> Uuid {
        let entity = upsert_entity_with_kind(pool, "en", "PR5 Person", "person")
            .await
            .unwrap();
        insert_claim(
            pool,
            &ClaimInsert {
                entity_id: entity,
                claim_kind: kind.into(),
                text: "PR5 claim".into(),
                epistemic_status: "attested".into(),
                relation_to_subject: "direct".into(),
                event_time: None,
                place_label: None,
                confidence: 0.5,
                canonical_event_id: None,
                debate_type: None,
                evidence_layer: None,
            },
        )
        .await
        .unwrap()
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn comment_valid_claim(pool: PgPool) {
        let u = user(&pool, "0xab5801a7d398351b8be11c439e05c5b3259aec9b").await;
        let c = claim(&pool, "theory").await;
        let dto = create_comment(&pool, u.id, c, "  Fragile hypothesis.  ", None)
            .await
            .unwrap();
        assert_eq!(dto.body.as_deref(), Some("Fragile hypothesis."));
        assert_eq!(dto.status, "active");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn comment_non_agora_rejected(pool: PgPool) {
        let u = user(&pool, "0xab5801a7d398351b8be11c439e05c5b3259aec9b").await;
        let c = claim(&pool, "life_event").await;
        let err = create_comment(&pool, u.id, c, "Nope", None)
            .await
            .unwrap_err();
        assert_eq!(err, CommentError::ClaimNotDiscussable);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn empty_and_too_long_body(pool: PgPool) {
        let u = user(&pool, "0xab5801a7d398351b8be11c439e05c5b3259aec9b").await;
        let c = claim(&pool, "controversy").await;
        assert_eq!(
            create_comment(&pool, u.id, c, "   ", None)
                .await
                .unwrap_err(),
            CommentError::InvalidCommentBody
        );
        let long = "é".repeat(4001);
        assert_eq!(
            create_comment(&pool, u.id, c, &long, None)
                .await
                .unwrap_err(),
            CommentError::InvalidCommentBody
        );
        let ok = "é".repeat(4000);
        create_comment(&pool, u.id, c, &ok, None).await.unwrap();
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn replies_one_level(pool: PgPool) {
        let a = user(&pool, "0xab5801a7d398351b8be11c439e05c5b3259aec9b").await;
        let b = user(&pool, "0x1111111111111111111111111111111111111111").await;
        let c = claim(&pool, "debate_stance").await;
        let root = create_comment(&pool, a.id, c, "Root", None).await.unwrap();
        let reply = create_comment(&pool, b.id, c, "Reply", Some(root.id))
            .await
            .unwrap();
        assert_eq!(reply.claim_id, c);
        let nested = create_comment(&pool, a.id, c, "Nested", Some(reply.id))
            .await
            .unwrap_err();
        assert_eq!(nested, CommentError::InvalidParentComment);
        let other = claim(&pool, "theory").await;
        let err = create_comment(&pool, a.id, other, "Wrong claim", Some(root.id))
            .await
            .unwrap_err();
        assert_eq!(err, CommentError::InvalidParentComment);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn edit_and_delete_ownership(pool: PgPool) {
        let a = user(&pool, "0xab5801a7d398351b8be11c439e05c5b3259aec9b").await;
        let b = user(&pool, "0x1111111111111111111111111111111111111111").await;
        let c = claim(&pool, "theory").await;
        let root = create_comment(&pool, a.id, c, "Root", None).await.unwrap();
        create_comment(&pool, b.id, c, "Stay", Some(root.id))
            .await
            .unwrap();
        let edited = patch_comment(&pool, a.id, root.id, "Edited").await.unwrap();
        assert_eq!(edited.body.as_deref(), Some("Edited"));
        assert!(edited.edited_at.is_some());
        let foreign = patch_comment(&pool, b.id, root.id, "Hack")
            .await
            .unwrap_err();
        assert_eq!(foreign, CommentError::CommentNotFound);
        delete_comment(&pool, a.id, root.id).await.unwrap();
        let (page, _) = list_claim_comments(&pool, c, None, None, None)
            .await
            .unwrap();
        assert_eq!(page[0].status, "deleted");
        assert!(page[0].body.is_none());
        assert_eq!(page[0].replies.as_ref().unwrap().len(), 1);
        assert_eq!(
            page[0].replies.as_ref().unwrap()[0].body.as_deref(),
            Some("Stay")
        );
        let react_deleted = add_comment_reaction(&pool, b.id, root.id, "relevant")
            .await
            .unwrap_err();
        assert_eq!(react_deleted, CommentError::CommentNotFound);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn reactions_idempotent_and_multi(pool: PgPool) {
        let a = user(&pool, "0xab5801a7d398351b8be11c439e05c5b3259aec9b").await;
        let c = claim(&pool, "theory").await;
        let root = create_comment(&pool, a.id, c, "Root", None).await.unwrap();
        let (_, created) = add_comment_reaction(&pool, a.id, root.id, "relevant")
            .await
            .unwrap();
        assert!(created);
        let (_, again) = add_comment_reaction(&pool, a.id, root.id, "relevant")
            .await
            .unwrap();
        assert!(!again);
        add_comment_reaction(&pool, a.id, root.id, "disagree")
            .await
            .unwrap();
        let dto = remove_comment_reaction(&pool, a.id, root.id, "relevant")
            .await
            .unwrap();
        assert_eq!(dto.reactions["relevant"], 0);
        assert_eq!(dto.reactions["disagree"], 1);
        assert_eq!(dto.my_reactions, vec!["disagree".to_string()]);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn list_batches_replies_and_reactions(pool: PgPool) {
        let a = user(&pool, "0xab5801a7d398351b8be11c439e05c5b3259aec9b").await;
        let c = claim(&pool, "theory").await;
        let mut last = None;
        for i in 0..3 {
            let root = create_comment(&pool, a.id, c, &format!("R{i}"), None)
                .await
                .unwrap();
            create_comment(&pool, a.id, c, "reply", Some(root.id))
                .await
                .unwrap();
            add_comment_reaction(&pool, a.id, root.id, "interesting")
                .await
                .unwrap();
            last = Some(root.id);
        }
        let (page, _) = list_claim_comments(&pool, c, Some(a.id), None, None)
            .await
            .unwrap();
        assert_eq!(page.len(), 3);
        assert_eq!(page[0].id, last.unwrap());
        assert_eq!(page[0].reply_count, Some(1));
        assert_eq!(page[0].replies.as_ref().unwrap()[0].body.as_deref(), Some("reply"));
        assert_eq!(page[0].reactions["interesting"], 1);
    }
}
