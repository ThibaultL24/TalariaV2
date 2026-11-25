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

/// Set to `1` before a deliberate schema apply. `serve` and `migrate` refuse
/// pending migrations without it, so a restart cannot change the schema.
pub const APPLY_MIGRATIONS_ENV: &str = "TALARIA_APPLY_MIGRATIONS";

pub fn migration_consent_granted() -> bool {
    std::env::var(APPLY_MIGRATIONS_ENV).ok().as_deref() == Some("1")
}

pub fn pending_migration_decision(pending: &[i64], consent: bool) -> Result<(), String> {
    if pending.is_empty() || consent {
        return Ok(());
    }
    let versions = pending
        .iter()
        .map(|version| version.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "refusing to apply pending migrations [{versions}] without {APPLY_MIGRATIONS_ENV}=1"
    ))
}

pub async fn pending_migration_versions(pool: &DbPool) -> anyhow::Result<Vec<i64>> {
    let migrator = sqlx::migrate!("../../migrations");
    let applied = applied_migration_versions(pool).await?;
    let mut pending: Vec<i64> = migrator
        .iter()
        .map(|migration| migration.version)
        .filter(|version| !applied.contains(version))
        .collect();
    pending.sort_unstable();
    Ok(pending)
}

async fn applied_migration_versions(
    pool: &DbPool,
) -> anyhow::Result<std::collections::HashSet<i64>> {
    let exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations') IS NOT NULL")
            .fetch_one(pool)
            .await?;
    if !exists {
        return Ok(std::collections::HashSet::new());
    }
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success")
            .fetch_all(pool)
            .await?;
    Ok(versions.into_iter().collect())
}

pub async fn run_migrations(pool: &DbPool) -> anyhow::Result<()> {
    let pending = pending_migration_versions(pool).await?;
    pending_migration_decision(&pending, migration_consent_granted())
        .map_err(anyhow::Error::msg)?;
    if pending.is_empty() {
        return Ok(());
    }
    // Re-resolved whenever migrations/ changes (see build.rs). ignore_missing
    // lets a DB that already applied 040 start even if an older embed omitted it.
    let mut migrator = sqlx::migrate!("../../migrations");
    migrator.set_ignore_missing(true);
    migrator.run(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_identity_migration_requires_explicit_consent() {
        assert!(pending_migration_decision(&[], false).is_ok());
        assert!(pending_migration_decision(&[53], true).is_ok());
        let refused = pending_migration_decision(&[53], false).unwrap_err();
        assert!(refused.contains("53"));
        assert!(refused.contains(APPLY_MIGRATIONS_ENV));
    }
}
