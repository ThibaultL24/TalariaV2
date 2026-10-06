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
    // Re-resolved whenever migrations/ changes (see build.rs). ignore_missing
    // lets a DB that already applied 040 start even if an older embed omitted it.
    let mut migrator = sqlx::migrate!("../../migrations");
    migrator.set_ignore_missing(true);
    migrator.run(pool).await?;
    Ok(())
}
