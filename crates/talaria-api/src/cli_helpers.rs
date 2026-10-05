// crates/talaria-api/src/cli_helpers.rs
//! Shared setup helpers for CLI command handlers.

use std::path::{Path, PathBuf};

use sqlx::PgPool;
use talaria_core::AppConfig;
use talaria_store::{connect, run_migrations, upsert_entity_with_kind};
use uuid::Uuid;

/// Resolve a repo-relative path when the CLI is run outside the workspace root (e.g. `web/`).
pub fn resolve_repo_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if path.is_file() || path.is_dir() {
        return path.to_path_buf();
    }
    let from_repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .join(path);
    if from_repo.is_file() || from_repo.is_dir() {
        return from_repo;
    }
    path.to_path_buf()
}

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
