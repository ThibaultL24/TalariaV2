// crates/talaria-api/src/interactions.rs
//! Off-chain Talaria interaction service. No Intuition deposits.

use serde::Serialize;
use sqlx::PgPool;
use talaria_store::{
    allowed_actions, count_active_comments_for_claims, count_arguments_for_claims,
    count_sources_for_claims, default_visibility, delete_owned_interaction, get_canonical_event,
    get_claim, get_corpus_document, get_entity_kind, is_allowed, is_check_violation,
    list_mine_for_targets, list_user_interactions, summarize_public_counts, upsert_interaction,
    InteractionAction, InteractionTargetType, InteractionUpsertResult, InteractionVisibility,
    UpsertInteraction, UserInteractionRow,
};
use uuid::Uuid;

pub const DEFAULT_LIST_LIMIT: i64 = 50;
pub const MAX_LIST_LIMIT: i64 = 100;
pub const MAX_SUMMARY_IDS: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractionError {
    Unauthenticated,
    InvalidAction,
    InvalidTargetType,
    InvalidActionForTarget,
    TargetNotFound,
    InteractionNotFound,
    InvalidVisibility,
    BatchTooLarge,
    DatabaseError,
}

impl InteractionError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthenticated => "unauthenticated",
            Self::InvalidAction => "invalid_action",
            Self::InvalidTargetType => "invalid_target_type",
            Self::InvalidActionForTarget => "invalid_action_for_target",
            Self::TargetNotFound => "target_not_found",
            Self::InteractionNotFound => "interaction_not_found",
            Self::InvalidVisibility => "invalid_visibility",
            Self::BatchTooLarge => "batch_too_large",
            Self::DatabaseError => "database_error",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::Unauthenticated => "Not authenticated",
            Self::InvalidAction => "Unknown interaction action",
            Self::InvalidTargetType => "Unknown or unsupported target type",
            Self::InvalidActionForTarget => "Action is not allowed for this target type",
            Self::TargetNotFound => "Interaction target does not exist",
            Self::InteractionNotFound => "Interaction not found",
            Self::InvalidVisibility => "Visibility must be public or private",
            Self::BatchTooLarge => "Too many target_ids (max 100)",
            Self::DatabaseError => "Internal database error",
        }
    }
}

fn map_db(err: anyhow::Error) -> InteractionError {
    tracing::error!(error = %err, "interaction database error");
    if is_check_violation(&err) {
        InteractionError::InvalidActionForTarget
    } else {
        InteractionError::DatabaseError
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct InteractionDto {
    pub id: Uuid,
    pub action: InteractionAction,
    pub target_type: InteractionTargetType,
    pub target_id: Uuid,
    pub visibility: InteractionVisibility,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl TryFrom<UserInteractionRow> for InteractionDto {
    type Error = InteractionError;

    fn try_from(row: UserInteractionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            action: InteractionAction::parse(&row.action_type)
                .ok_or(InteractionError::InvalidAction)?,
            target_type: InteractionTargetType::parse(&row.target_type)
                .ok_or(InteractionError::InvalidTargetType)?,
            target_id: row.target_id,
            visibility: InteractionVisibility::parse(&row.visibility)
                .ok_or(InteractionError::InvalidVisibility)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

pub async fn resolve_interaction_target(
    pool: &PgPool,
    target: InteractionTargetType,
    target_id: Uuid,
) -> Result<(), InteractionError> {
    if !target.is_supported() {
        return Err(InteractionError::InvalidTargetType);
    }
    match target {
        InteractionTargetType::Person => match get_entity_kind(pool, target_id)
            .await
            .map_err(map_db)?
        {
            Some(kind) if kind == "person" => Ok(()),
            _ => Err(InteractionError::TargetNotFound),
        },
        InteractionTargetType::Place => match get_entity_kind(pool, target_id)
            .await
            .map_err(map_db)?
        {
            Some(kind) if kind == "place" => Ok(()),
            _ => Err(InteractionError::TargetNotFound),
        },
        InteractionTargetType::Claim => {
            if get_claim(pool, target_id).await.map_err(map_db)?.is_some() {
                Ok(())
            } else {
                Err(InteractionError::TargetNotFound)
            }
        }
        InteractionTargetType::Event => {
            if get_canonical_event(pool, target_id)
                .await
                .map_err(map_db)?
                .is_some()
            {
                Ok(())
            } else {
                Err(InteractionError::TargetNotFound)
            }
        }
        InteractionTargetType::Source => {
            if get_corpus_document(pool, target_id)
                .await
                .map_err(map_db)?
                .is_some()
            {
                Ok(())
            } else {
                Err(InteractionError::TargetNotFound)
            }
        }
        InteractionTargetType::Route => Err(InteractionError::InvalidTargetType),
    }
}

pub fn parse_action(raw: &str) -> Result<InteractionAction, InteractionError> {
    InteractionAction::parse(raw).ok_or(InteractionError::InvalidAction)
}

pub fn parse_target_type(raw: &str) -> Result<InteractionTargetType, InteractionError> {
    let t = InteractionTargetType::parse(raw).ok_or(InteractionError::InvalidTargetType)?;
    if !t.is_supported() {
        return Err(InteractionError::InvalidTargetType);
    }
    Ok(t)
}

pub fn parse_visibility(raw: Option<&str>) -> Result<Option<InteractionVisibility>, InteractionError> {
    match raw {
        None => Ok(None),
        Some(s) => InteractionVisibility::parse(s)
            .map(Some)
            .ok_or(InteractionError::InvalidVisibility),
    }
}

pub async fn create_interaction(
    pool: &PgPool,
    user_id: Uuid,
    action: InteractionAction,
    target_type: InteractionTargetType,
    target_id: Uuid,
    visibility: Option<InteractionVisibility>,
) -> Result<InteractionUpsertResult, InteractionError> {
    if !target_type.is_supported() {
        return Err(InteractionError::InvalidTargetType);
    }
    if !is_allowed(action, target_type) {
        return Err(InteractionError::InvalidActionForTarget);
    }
    resolve_interaction_target(pool, target_type, target_id).await?;
    let visibility = visibility.unwrap_or_else(|| default_visibility(action));
    upsert_interaction(
        pool,
        &UpsertInteraction {
            user_id,
            action,
            target_type,
            target_id,
            visibility,
        },
    )
    .await
    .map_err(map_db)
}

pub async fn delete_interaction(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
) -> Result<(), InteractionError> {
    if delete_owned_interaction(pool, id, user_id)
        .await
        .map_err(map_db)?
    {
        Ok(())
    } else {
        Err(InteractionError::InteractionNotFound)
    }
}

pub fn clamp_list_limit(limit: Option<i64>) -> i64 {
    match limit {
        None => DEFAULT_LIST_LIMIT,
        Some(n) if n < 1 => 1,
        Some(n) if n > MAX_LIST_LIMIT => MAX_LIST_LIMIT,
        Some(n) => n,
    }
}

pub fn decode_list_cursor(
    cursor: Option<&str>,
) -> (Option<chrono::DateTime<chrono::Utc>>, Option<Uuid>) {
    let Some(raw) = cursor.map(str::trim).filter(|s| !s.is_empty()) else {
        return (None, None);
    };
    let mut parts = raw.splitn(2, '_');
    let ts = parts
        .next()
        .and_then(|s| s.parse::<i64>().ok())
        .and_then(chrono::DateTime::from_timestamp_micros);
    let id = parts.next().and_then(|s| Uuid::parse_str(s).ok());
    (ts, id)
}

pub fn encode_list_cursor(row: &UserInteractionRow) -> String {
    format!("{}_{}", row.created_at.timestamp_micros(), row.id)
}

pub async fn list_mine(
    pool: &PgPool,
    user_id: Uuid,
    action: Option<InteractionAction>,
    target_type: Option<InteractionTargetType>,
    cursor: Option<&str>,
    limit: Option<i64>,
) -> Result<(Vec<InteractionDto>, Option<String>), InteractionError> {
    let limit = clamp_list_limit(limit);
    let (cursor_ts, cursor_id) = decode_list_cursor(cursor);
    let rows = list_user_interactions(
        pool,
        user_id,
        action,
        target_type,
        cursor_ts,
        cursor_id,
        limit + 1,
    )
    .await
    .map_err(map_db)?;
    let next = if rows.len() as i64 > limit {
        rows.get(limit as usize - 1)
            .map(encode_list_cursor)
    } else {
        None
    };
    let items = rows
        .into_iter()
        .take(limit as usize)
        .map(InteractionDto::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok((items, next))
}

#[derive(Debug, Clone, Serialize)]
pub struct InteractionSummary {
    pub target_id: Uuid,
    pub counts: serde_json::Map<String, serde_json::Value>,
    pub comment_count: i64,
    #[serde(default)]
    pub argument_count: i64,
    #[serde(default)]
    pub source_count: i64,
    pub mine: Vec<InteractionAction>,
}

pub async fn summarize_targets(
    pool: &PgPool,
    target_type: InteractionTargetType,
    target_ids: Vec<Uuid>,
    user_id: Option<Uuid>,
) -> Result<Vec<InteractionSummary>, InteractionError> {
    if !target_type.is_supported() {
        return Err(InteractionError::InvalidTargetType);
    }
    if target_ids.len() > MAX_SUMMARY_IDS {
        return Err(InteractionError::BatchTooLarge);
    }
    let counts = summarize_public_counts(pool, target_type, &target_ids)
        .await
        .map_err(map_db)?;
    let mine_rows = if let Some(uid) = user_id {
        list_mine_for_targets(pool, uid, target_type, &target_ids)
            .await
            .map_err(map_db)?
    } else {
        vec![]
    };

    let mut count_map: std::collections::HashMap<(Uuid, String), i64> =
        std::collections::HashMap::new();
    for row in counts {
        count_map.insert((row.target_id, row.action_type), row.n);
    }
    let mut mine_map: std::collections::HashMap<Uuid, Vec<InteractionAction>> =
        std::collections::HashMap::new();
    for row in mine_rows {
        if let Some(action) = InteractionAction::parse(&row.action_type) {
            mine_map.entry(row.target_id).or_default().push(action);
        }
    }

    // Batch counts for the Agora list: one query each for comments, arguments, sources.
    // Never N+1 per claim card. Detail payloads stay on lazy GET /claims/{id}/...
    let comment_rows = if target_type == InteractionTargetType::Claim {
        count_active_comments_for_claims(pool, &target_ids)
            .await
            .map_err(map_db)?
    } else {
        vec![]
    };
    let mut comment_map: std::collections::HashMap<Uuid, i64> = std::collections::HashMap::new();
    for row in comment_rows {
        comment_map.insert(row.claim_id, row.n);
    }
    let mut argument_map: std::collections::HashMap<Uuid, i64> = std::collections::HashMap::new();
    let mut source_map: std::collections::HashMap<Uuid, i64> = std::collections::HashMap::new();
    if target_type == InteractionTargetType::Claim {
        for row in count_arguments_for_claims(pool, &target_ids)
            .await
            .map_err(map_db)?
        {
            argument_map.insert(row.claim_id, row.n);
        }
        for row in count_sources_for_claims(pool, &target_ids)
            .await
            .map_err(map_db)?
        {
            source_map.insert(row.claim_id, row.n);
        }
    }

    let allowed = allowed_actions(target_type);
    Ok(target_ids
        .into_iter()
        .map(|target_id| {
            let mut counts = serde_json::Map::new();
            for action in allowed {
                let n = count_map
                    .get(&(target_id, action.as_str().to_string()))
                    .copied()
                    .unwrap_or(0);
                counts.insert(
                    action.as_str().to_string(),
                    serde_json::Value::Number(n.into()),
                );
            }
            InteractionSummary {
                target_id,
                counts,
                comment_count: comment_map.get(&target_id).copied().unwrap_or(0),
                argument_count: argument_map.get(&target_id).copied().unwrap_or(0),
                source_count: source_map.get(&target_id).copied().unwrap_or(0),
                mine: mine_map.remove(&target_id).unwrap_or_default(),
            }
        })
        .collect())
}
