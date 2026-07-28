// crates/talaria-store/src/judgments.rs
use sqlx::PgPool;
use uuid::Uuid;

pub async fn insert_judgment(
    pool: &PgPool,
    phrase_candidate_id: Uuid,
    judge_kind: &str,
    score: f64,
    label: &str,
    result_json: serde_json::Value,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO candidate_judgments (phrase_candidate_id, judge_kind, score, label, result_json)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(phrase_candidate_id)
    .bind(judge_kind)
    .bind(score)
    .bind(label)
    .bind(result_json)
    .fetch_one(pool)
    .await?;

    Ok(id)
}
