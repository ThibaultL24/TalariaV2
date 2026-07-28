// crates/talaria-store/src/pool.rs
use sqlx::{postgres::PgPoolOptions, PgPool};
use talaria_core::AppConfig;

pub type DbPool = PgPool;

pub async fn connect(config: &AppConfig) -> anyhow::Result<DbPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &DbPool) -> anyhow::Result<()> {
    sqlx::migrate!("../../migrations").run(pool).await?;
    Ok(())
}
