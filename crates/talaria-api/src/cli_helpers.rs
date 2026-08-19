// crates/talaria-api/src/cli_helpers.rs
//! Shared setup helpers for CLI command handlers.

use sqlx::PgPool;
use talaria_core::AppConfig;
use talaria_store::{connect, run_migrations, upsert_entity_with_kind};
use uuid::Uuid;

/// Open the database, run pending migrations, and upsert the subject entity.
/// Returns `(pool, subject_entity_id)` ready for use by any ingest handler.
pub async fn open_db_for_subject(
    config: &AppConfig,
    subject: &str,
    entity_kind: &str,
) -> anyhow::Result<(PgPool, Uuid)> {
    let pool = connect(config).await?;
    run_migrations(&pool).await?;
    let subject_id =
        upsert_entity_with_kind(&pool, &config.wiki_lang, subject, entity_kind).await?;
    Ok((pool, subject_id))
}
