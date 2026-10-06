// crates/talaria-store/src/user_interactions.rs
//! Persistence for stateful user interactions. No HTTP.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::interaction_types::{
    InteractionAction, InteractionTargetType, InteractionVisibility,
};
use crate::is_unique_violation;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserInteractionRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub action_type: String,
    pub target_type: String,
    pub target_id: Uuid,
    pub visibility: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UpsertInteraction {
    pub user_id: Uuid,
    pub action: InteractionAction,
    pub target_type: InteractionTargetType,
    pub target_id: Uuid,
    pub visibility: InteractionVisibility,
}

#[derive(Debug, Clone)]
pub struct InteractionUpsertResult {
    pub row: UserInteractionRow,
    pub created: bool,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InteractionCountRow {
    pub target_id: Uuid,
    pub action_type: String,
    pub n: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InteractionMineRow {
    pub target_id: Uuid,
    pub action_type: String,
}

const SELECT_COLS: &str = r#"
    id, user_id, action_type, target_type, target_id, visibility, created_at, updated_at
"#;

pub async fn upsert_interaction(
    pool: &PgPool,
    input: &UpsertInteraction,
) -> anyhow::Result<InteractionUpsertResult> {
    if input.action.is_epistemic_stance() {
        return replace_claim_stance(pool, input).await;
    }
    upsert_non_stance(pool, input).await
}

async fn upsert_non_stance(
    pool: &PgPool,
    input: &UpsertInteraction,
) -> anyhow::Result<InteractionUpsertResult> {
    if let Some(existing) = find_interaction(
        pool,
        input.user_id,
        input.action,
        input.target_type,
        input.target_id,
    )
    .await?
    {
        let row = update_visibility(pool, existing.id, input.visibility).await?;
        return Ok(InteractionUpsertResult {
            row,
            created: false,
        });
    }
    match insert_interaction(pool, input).await {
        Ok(row) => Ok(InteractionUpsertResult {
            row,
            created: true,
        }),
        Err(err) if is_unique_violation(&err) => {
            let row = find_interaction(
                pool,
                input.user_id,
                input.action,
                input.target_type,
                input.target_id,
            )
            .await?
            .ok_or_else(|| anyhow::anyhow!("interaction missing after unique conflict"))?;
            let row = update_visibility(pool, row.id, input.visibility).await?;
            Ok(InteractionUpsertResult {
                row,
                created: false,
            })
        }
        Err(err) => Err(err),
    }
}

/// Deletes any other claim stance for this user, then upserts the requested stance.
pub async fn replace_claim_stance(
    pool: &PgPool,
    input: &UpsertInteraction,
) -> anyhow::Result<InteractionUpsertResult> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"
        DELETE FROM user_interactions
        WHERE user_id = $1
          AND target_type = 'claim'
          AND target_id = $2
          AND action_type IN ('support', 'dispute', 'uncertain')
          AND action_type <> $3
        "#,
    )
    .bind(input.user_id)
    .bind(input.target_id)
    .bind(input.action.as_str())
    .execute(&mut *tx)
    .await?;

    let existing: Option<UserInteractionRow> = sqlx::query_as(
        &format!(
            r#"
            SELECT {SELECT_COLS}
            FROM user_interactions
            WHERE user_id = $1
              AND action_type = $2
              AND target_type = 'claim'
              AND target_id = $3
            "#
        ),
    )
    .bind(input.user_id)
    .bind(input.action.as_str())
    .bind(input.target_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (row, created) = if let Some(existing) = existing {
        let row = sqlx::query_as::<_, UserInteractionRow>(&format!(
            r#"
            UPDATE user_interactions
            SET visibility = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(existing.id)
        .bind(input.visibility.as_str())
        .fetch_one(&mut *tx)
        .await?;
        (row, false)
    } else {
        let row = sqlx::query_as::<_, UserInteractionRow>(&format!(
            r#"
            INSERT INTO user_interactions (
                user_id, action_type, target_type, target_id, visibility
            )
            VALUES ($1, $2, $3, $4, $5)
            RETURNING {SELECT_COLS}
            "#
        ))
        .bind(input.user_id)
        .bind(input.action.as_str())
        .bind(input.target_type.as_str())
        .bind(input.target_id)
        .bind(input.visibility.as_str())
        .fetch_one(&mut *tx)
        .await?;
        (row, true)
    };
    tx.commit().await?;
    Ok(InteractionUpsertResult { row, created })
}

async fn insert_interaction(
    pool: &PgPool,
    input: &UpsertInteraction,
) -> anyhow::Result<UserInteractionRow> {
    let row = sqlx::query_as::<_, UserInteractionRow>(&format!(
        r#"
        INSERT INTO user_interactions (
            user_id, action_type, target_type, target_id, visibility
        )
        VALUES ($1, $2, $3, $4, $5)
        RETURNING {SELECT_COLS}
        "#
    ))
    .bind(input.user_id)
    .bind(input.action.as_str())
    .bind(input.target_type.as_str())
    .bind(input.target_id)
    .bind(input.visibility.as_str())
    .fetch_one(pool)
    .await?;
    Ok(row)
}

async fn update_visibility(
    pool: &PgPool,
    id: Uuid,
    visibility: InteractionVisibility,
) -> anyhow::Result<UserInteractionRow> {
    let row = sqlx::query_as::<_, UserInteractionRow>(&format!(
        r#"
        UPDATE user_interactions
        SET visibility = $2, updated_at = NOW()
        WHERE id = $1
        RETURNING {SELECT_COLS}
        "#
    ))
    .bind(id)
    .bind(visibility.as_str())
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn find_interaction(
    pool: &PgPool,
    user_id: Uuid,
    action: InteractionAction,
    target_type: InteractionTargetType,
    target_id: Uuid,
) -> anyhow::Result<Option<UserInteractionRow>> {
    let row = sqlx::query_as::<_, UserInteractionRow>(&format!(
        r#"
        SELECT {SELECT_COLS}
        FROM user_interactions
        WHERE user_id = $1
          AND action_type = $2
          AND target_type = $3
          AND target_id = $4
        "#
    ))
    .bind(user_id)
    .bind(action.as_str())
    .bind(target_type.as_str())
    .bind(target_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn get_interaction(
    pool: &PgPool,
    id: Uuid,
) -> anyhow::Result<Option<UserInteractionRow>> {
    let row = sqlx::query_as::<_, UserInteractionRow>(&format!(
        r#"SELECT {SELECT_COLS} FROM user_interactions WHERE id = $1"#
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Deletes only if owned by `user_id`. Returns false when missing or foreign.
pub async fn delete_owned_interaction(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> anyhow::Result<bool> {
    let n = sqlx::query(
        r#"
        DELETE FROM user_interactions
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(n == 1)
}

pub async fn list_user_interactions(
    pool: &PgPool,
    user_id: Uuid,
    action: Option<InteractionAction>,
    target_type: Option<InteractionTargetType>,
    cursor_created_at: Option<DateTime<Utc>>,
    cursor_id: Option<Uuid>,
    limit: i64,
) -> anyhow::Result<Vec<UserInteractionRow>> {
    let rows = sqlx::query_as::<_, UserInteractionRow>(&format!(
        r#"
        SELECT {SELECT_COLS}
        FROM user_interactions
        WHERE user_id = $1
          AND ($2::text IS NULL OR action_type = $2)
          AND ($3::text IS NULL OR target_type = $3)
          AND (
            $4::timestamptz IS NULL
            OR (created_at, id) < ($4, $5)
          )
        ORDER BY created_at DESC, id DESC
        LIMIT $6
        "#
    ))
    .bind(user_id)
    .bind(action.map(|a| a.as_str()))
    .bind(target_type.map(|t| t.as_str()))
    .bind(cursor_created_at)
    .bind(cursor_id.unwrap_or(Uuid::nil()))
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// One aggregated query for public counts. Never N×SELECT.
pub async fn summarize_public_counts(
    pool: &PgPool,
    target_type: InteractionTargetType,
    target_ids: &[Uuid],
) -> anyhow::Result<Vec<InteractionCountRow>> {
    if target_ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query_as::<_, InteractionCountRow>(
        r#"
        SELECT target_id, action_type, COUNT(*)::bigint AS n
        FROM user_interactions
        WHERE target_type = $1
          AND target_id = ANY($2)
          AND visibility = 'public'
        GROUP BY target_id, action_type
        "#,
    )
    .bind(target_type.as_str())
    .bind(target_ids)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_mine_for_targets(
    pool: &PgPool,
    user_id: Uuid,
    target_type: InteractionTargetType,
    target_ids: &[Uuid],
) -> anyhow::Result<Vec<InteractionMineRow>> {
    if target_ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query_as::<_, InteractionMineRow>(
        r#"
        SELECT target_id, action_type
        FROM user_interactions
        WHERE user_id = $1
          AND target_type = $2
          AND target_id = ANY($3)
        "#,
    )
    .bind(user_id)
    .bind(target_type.as_str())
    .bind(target_ids)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction_types::is_allowed;
    use crate::{resolve_or_create_evm_user, NormalizedEvmAddress};

    async fn user(pool: &PgPool, hex: &str) -> Uuid {
        let addr = NormalizedEvmAddress::parse(hex).unwrap();
        resolve_or_create_evm_user(pool, &addr).await.unwrap().id
    }

    fn input(
        user_id: Uuid,
        action: InteractionAction,
        target_type: InteractionTargetType,
        target_id: Uuid,
        visibility: InteractionVisibility,
    ) -> UpsertInteraction {
        UpsertInteraction {
            user_id,
            action,
            target_type,
            target_id,
            visibility,
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn save_same_target_twice_one_row(pool: PgPool) {
        let u = user(&pool, "0x1111111111111111111111111111111111111111").await;
        let t = Uuid::new_v4();
        let a = upsert_interaction(
            &pool,
            &input(
                u,
                InteractionAction::Save,
                InteractionTargetType::Event,
                t,
                InteractionVisibility::Private,
            ),
        )
        .await
        .unwrap();
        assert!(a.created);
        let b = upsert_interaction(
            &pool,
            &input(
                u,
                InteractionAction::Save,
                InteractionTargetType::Event,
                t,
                InteractionVisibility::Private,
            ),
        )
        .await
        .unwrap();
        assert!(!b.created);
        assert_eq!(a.row.id, b.row.id);
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM user_interactions WHERE user_id = $1",
        )
        .bind(u)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, 1);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn two_users_save_same_target_two_rows(pool: PgPool) {
        let a = user(&pool, "0x2222222222222222222222222222222222222222").await;
        let b = user(&pool, "0x3333333333333333333333333333333333333333").await;
        let t = Uuid::new_v4();
        upsert_interaction(
            &pool,
            &input(
                a,
                InteractionAction::Save,
                InteractionTargetType::Event,
                t,
                InteractionVisibility::Private,
            ),
        )
        .await
        .unwrap();
        upsert_interaction(
            &pool,
            &input(
                b,
                InteractionAction::Save,
                InteractionTargetType::Event,
                t,
                InteractionVisibility::Private,
            ),
        )
        .await
        .unwrap();
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM user_interactions WHERE target_id = $1",
        )
        .bind(t)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, 2);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn support_then_dispute_one_stance(pool: PgPool) {
        let u = user(&pool, "0x4444444444444444444444444444444444444444").await;
        let claim = Uuid::new_v4();
        upsert_interaction(
            &pool,
            &input(
                u,
                InteractionAction::Support,
                InteractionTargetType::Claim,
                claim,
                InteractionVisibility::Public,
            ),
        )
        .await
        .unwrap();
        let next = upsert_interaction(
            &pool,
            &input(
                u,
                InteractionAction::Dispute,
                InteractionTargetType::Claim,
                claim,
                InteractionVisibility::Public,
            ),
        )
        .await
        .unwrap();
        assert_eq!(next.row.action_type, "dispute");
        let n: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint FROM user_interactions
            WHERE user_id = $1 AND target_id = $2
              AND action_type IN ('support','dispute','uncertain')
            "#,
        )
        .bind(u)
        .bind(claim)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, 1);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn support_then_uncertain_one_stance(pool: PgPool) {
        let u = user(&pool, "0x5555555555555555555555555555555555555555").await;
        let claim = Uuid::new_v4();
        upsert_interaction(
            &pool,
            &input(
                u,
                InteractionAction::Support,
                InteractionTargetType::Claim,
                claim,
                InteractionVisibility::Public,
            ),
        )
        .await
        .unwrap();
        upsert_interaction(
            &pool,
            &input(
                u,
                InteractionAction::Uncertain,
                InteractionTargetType::Claim,
                claim,
                InteractionVisibility::Public,
            ),
        )
        .await
        .unwrap();
        let row: String = sqlx::query_scalar(
            r#"
            SELECT action_type FROM user_interactions
            WHERE user_id = $1 AND target_id = $2
              AND action_type IN ('support','dispute','uncertain')
            "#,
        )
        .bind(u)
        .bind(claim)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row, "uncertain");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn save_and_support_same_claim(pool: PgPool) {
        let u = user(&pool, "0x6666666666666666666666666666666666666666").await;
        let claim = Uuid::new_v4();
        upsert_interaction(
            &pool,
            &input(
                u,
                InteractionAction::Save,
                InteractionTargetType::Claim,
                claim,
                InteractionVisibility::Private,
            ),
        )
        .await
        .unwrap();
        upsert_interaction(
            &pool,
            &input(
                u,
                InteractionAction::Support,
                InteractionTargetType::Claim,
                claim,
                InteractionVisibility::Public,
            ),
        )
        .await
        .unwrap();
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM user_interactions WHERE user_id = $1 AND target_id = $2",
        )
        .bind(u)
        .bind(claim)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, 2);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn follow_claim_and_support_person_rejected(pool: PgPool) {
        let u = user(&pool, "0x7777777777777777777777777777777777777777").await;
        assert!(!is_allowed(
            InteractionAction::Follow,
            InteractionTargetType::Claim
        ));
        let err = upsert_interaction(
            &pool,
            &input(
                u,
                InteractionAction::Follow,
                InteractionTargetType::Claim,
                Uuid::new_v4(),
                InteractionVisibility::Private,
            ),
        )
        .await
        .unwrap_err();
        assert!(crate::is_check_violation(&err));
        let err = upsert_interaction(
            &pool,
            &input(
                u,
                InteractionAction::Support,
                InteractionTargetType::Person,
                Uuid::new_v4(),
                InteractionVisibility::Public,
            ),
        )
        .await
        .unwrap_err();
        assert!(crate::is_check_violation(&err));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn private_support_absent_from_public_counts(pool: PgPool) {
        let u = user(&pool, "0x8888888888888888888888888888888888888888").await;
        let claim = Uuid::new_v4();
        upsert_interaction(
            &pool,
            &input(
                u,
                InteractionAction::Support,
                InteractionTargetType::Claim,
                claim,
                InteractionVisibility::Private,
            ),
        )
        .await
        .unwrap();
        let counts = summarize_public_counts(&pool, InteractionTargetType::Claim, &[claim])
            .await
            .unwrap();
        assert!(counts.is_empty());
        let mine = list_mine_for_targets(&pool, u, InteractionTargetType::Claim, &[claim])
            .await
            .unwrap();
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].action_type, "support");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn public_support_present_in_counts(pool: PgPool) {
        let u = user(&pool, "0x9999999999999999999999999999999999999999").await;
        let claim = Uuid::new_v4();
        upsert_interaction(
            &pool,
            &input(
                u,
                InteractionAction::Support,
                InteractionTargetType::Claim,
                claim,
                InteractionVisibility::Public,
            ),
        )
        .await
        .unwrap();
        let counts = summarize_public_counts(&pool, InteractionTargetType::Claim, &[claim])
            .await
            .unwrap();
        assert_eq!(counts.len(), 1);
        assert_eq!(counts[0].n, 1);
        assert_eq!(counts[0].action_type, "support");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn batch_summary_one_grouped_query(pool: PgPool) {
        let u = user(&pool, "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").await;
        let mut ids = Vec::new();
        for _ in 0..100 {
            let id = Uuid::new_v4();
            ids.push(id);
            upsert_interaction(
                &pool,
                &input(
                    u,
                    InteractionAction::Support,
                    InteractionTargetType::Claim,
                    id,
                    InteractionVisibility::Public,
                ),
            )
            .await
            .unwrap();
        }
        let counts = summarize_public_counts(&pool, InteractionTargetType::Claim, &ids)
            .await
            .unwrap();
        assert_eq!(counts.len(), 100);
        assert!(counts.iter().all(|r| r.n == 1 && r.action_type == "support"));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn delete_owned_not_foreign(pool: PgPool) {
        let a = user(&pool, "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").await;
        let b = user(&pool, "0xcccccccccccccccccccccccccccccccccccccccc").await;
        let t = Uuid::new_v4();
        let row = upsert_interaction(
            &pool,
            &input(
                a,
                InteractionAction::Save,
                InteractionTargetType::Event,
                t,
                InteractionVisibility::Private,
            ),
        )
        .await
        .unwrap()
        .row;
        assert!(!delete_owned_interaction(&pool, row.id, b).await.unwrap());
        assert!(delete_owned_interaction(&pool, row.id, a).await.unwrap());
        assert!(get_interaction(&pool, row.id).await.unwrap().is_none());
    }
}
