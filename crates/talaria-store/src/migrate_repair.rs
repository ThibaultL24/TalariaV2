// crates/talaria-store/src/migrate_repair.rs
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha384};
use sqlx::PgPool;

fn migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../migrations")
}

fn parse_migration_path(path: &Path) -> Option<(i64, String)> {
    let name = path.file_name()?.to_string_lossy();
    let stem = name.strip_suffix(".sql")?;
    let (ver_str, rest) = stem.split_once('_')?;
    let version = ver_str.parse::<i64>().ok()?;
    let description = rest.replace('_', " ");
    Some((version, description))
}

fn file_checksum(bytes: &[u8]) -> Vec<u8> {
    Sha384::digest(bytes).to_vec()
}

/// Update `_sqlx_migrations.checksum` when a migration file was edited after apply (dev only).
pub async fn repair_migration_checksums(
    pool: &PgPool,
    only_version: Option<i64>,
) -> anyhow::Result<Vec<i64>> {
    let dir = migrations_dir();
    if !dir.is_dir() {
        anyhow::bail!("migrations directory not found: {}", dir.display());
    }

    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "sql"))
        .collect();
    paths.sort();

    let mut repaired = Vec::new();
    for path in paths {
        let (version, _description) = parse_migration_path(&path)
            .ok_or_else(|| anyhow::anyhow!("invalid migration filename: {}", path.display()))?;
        if only_version.is_some_and(|v| v != version) {
            continue;
        }

        let bytes = std::fs::read(&path)?;
        let expected = file_checksum(&bytes);

        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT checksum FROM _sqlx_migrations WHERE version = $1")
                .bind(version)
                .fetch_optional(pool)
                .await?;

        let Some((stored,)) = row else {
            continue;
        };

        if stored != expected {
            sqlx::query("UPDATE _sqlx_migrations SET checksum = $1 WHERE version = $2")
                .bind(&expected)
                .bind(version)
                .execute(pool)
                .await?;
            repaired.push(version);
        }
    }

    Ok(repaired)
}
