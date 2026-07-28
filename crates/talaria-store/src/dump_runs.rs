// crates/talaria-store/src/dump_runs.rs
use sqlx::PgPool;
use uuid::Uuid;

pub async fn start_dump_run(
    pool: &PgPool,
    dump_path: &str,
    wiki_lang: &str,
) -> anyhow::Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO dump_runs (dump_path, wiki_lang, status)
        VALUES ($1, $2, 'running')
        RETURNING id
        "#,
    )
    .bind(dump_path)
    .bind(wiki_lang)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn finish_dump_run(
    pool: &PgPool,
    run_id: Uuid,
    pages_indexed: i32,
    status: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE dump_runs
        SET status = $2, pages_indexed = $3, ended_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(run_id)
    .bind(status)
    .bind(pages_indexed)
    .execute(pool)
    .await?;

    Ok(())
}
