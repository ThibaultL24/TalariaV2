// crates/talaria-api/src/rebuild.rs
//! Explicit operator wipe for the unified person pipeline. Never run from a migration.

use std::collections::HashMap;
use std::path::Path;
use std::process;

use serde::Serialize;
use sqlx::{PgPool, Postgres, Transaction};
use talaria_core::AppConfig;
use talaria_store::connect;
use uuid::Uuid;

#[derive(Debug, Serialize)]
struct RebuildCounts {
    canonical_events: i64,
    event_evidence: i64,
    event_candidates: i64,
    lotd_entities: i64,
    duplicate_qids: i64,
}

#[derive(Debug, Serialize)]
struct RebuildManifest {
    counts: RebuildCounts,
    sampled_ids: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct EntityDupRow {
    id: Uuid,
    qid: String,
    canonical_name: Option<String>,
    event_count: i64,
}

pub async fn rebuild_person_pipeline(
    config: &AppConfig,
    confirm: bool,
    manifest_path: &Path,
) -> anyhow::Result<()> {
    let pool = connect(config).await?;
    let counts = collect_counts(&pool).await?;
    print_counts(&counts);

    if !confirm {
        eprintln!("refusing wipe: pass --confirm-destruction to merge QIDs and TRUNCATE events");
        process::exit(2);
    }

    let sampled_ids = sample_ids(&pool).await?;
    let manifest = RebuildManifest {
        counts,
        sampled_ids,
    };
    if let Some(parent) = manifest_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
    println!("wrote backup manifest {}", manifest_path.display());

    let mut tx = pool.begin().await?;
    merge_duplicate_qids(&mut tx).await?;
    sqlx::query("DELETE FROM entities WHERE canonical_name ILIKE '%LotD%'")
        .execute(&mut *tx)
        .await?;

    // TRUNCATE ... CASCADE would also wipe soft_claims (FK SET NULL still CASCADE-truncates).
    detach_canonical_event_fks(&mut tx).await?;
    sqlx::query("TRUNCATE canonical_events, event_evidence")
        .execute(&mut *tx)
        .await?;
    null_event_candidate_fks(&mut tx).await?;
    sqlx::query("TRUNCATE event_candidates")
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_entities_qid ON entities (qid) WHERE qid IS NOT NULL",
    )
    .execute(&mut *tx)
    .await
    .map_err(|err| anyhow::anyhow!("uq_entities_qid still cannot be created: {err}"))?;

    let remaining_events: i64 = sqlx::query_scalar("SELECT count(*) FROM canonical_events")
        .fetch_one(&mut *tx)
        .await?;
    anyhow::ensure!(
        remaining_events == 0,
        "canonical_events count is {remaining_events} after TRUNCATE"
    );

    let remaining_dups: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (
             SELECT qid FROM entities WHERE qid IS NOT NULL GROUP BY qid HAVING count(*) > 1
         ) s",
    )
    .fetch_one(&mut *tx)
    .await?;
    anyhow::ensure!(
        remaining_dups == 0,
        "{remaining_dups} duplicate qid groups remain after merge"
    );

    ensure_deferred_pipeline_constraints(&mut tx).await?;

    tx.commit().await?;

    println!("rebuild complete: canonical_events=0, unique qid index present");
    println!("next: ingest via the search bar, or `talaria` person ingest (this command does not reingest)");
    Ok(())
}

async fn collect_counts(pool: &PgPool) -> anyhow::Result<RebuildCounts> {
    Ok(RebuildCounts {
        canonical_events: count_star(pool, "canonical_events").await?,
        event_evidence: count_star(pool, "event_evidence").await?,
        event_candidates: count_star(pool, "event_candidates").await?,
        lotd_entities: sqlx::query_scalar(
            "SELECT count(*) FROM entities WHERE canonical_name ILIKE '%LotD%'",
        )
        .fetch_one(pool)
        .await?,
        duplicate_qids: sqlx::query_scalar(
            "SELECT count(*) FROM (
                 SELECT qid FROM entities WHERE qid IS NOT NULL GROUP BY qid HAVING count(*) > 1
             ) s",
        )
        .fetch_one(pool)
        .await?,
    })
}

fn print_counts(counts: &RebuildCounts) {
    println!("canonical_events\t{}", counts.canonical_events);
    println!("event_evidence\t{}", counts.event_evidence);
    println!("event_candidates\t{}", counts.event_candidates);
    println!("lotd_entities\t{}", counts.lotd_entities);
    println!("duplicate_qids\t{}", counts.duplicate_qids);
}

async fn count_star(pool: &PgPool, table: &str) -> anyhow::Result<i64> {
    let sql = format!("SELECT count(*) FROM {table}");
    Ok(sqlx::query_scalar(&sql).fetch_one(pool).await?)
}

async fn sample_ids(pool: &PgPool) -> anyhow::Result<Vec<String>> {
    let mut ids = Vec::new();
    push_id_samples(
        pool,
        "SELECT id::text FROM canonical_events LIMIT 8",
        &mut ids,
    )
    .await?;
    push_id_samples(
        pool,
        "SELECT id::text FROM event_candidates LIMIT 8",
        &mut ids,
    )
    .await?;
    push_id_samples(
        pool,
        "SELECT id::text FROM entities WHERE canonical_name ILIKE '%LotD%' LIMIT 8",
        &mut ids,
    )
    .await?;
    push_id_samples(
        pool,
        "SELECT id::text FROM entities WHERE qid IN (
             SELECT qid FROM entities WHERE qid IS NOT NULL GROUP BY qid HAVING count(*) > 1
         ) LIMIT 16",
        &mut ids,
    )
    .await?;
    Ok(ids)
}

async fn push_id_samples(
    pool: &PgPool,
    sql: &str,
    ids: &mut Vec<String>,
) -> anyhow::Result<()> {
    let rows: Vec<(String,)> = sqlx::query_as(sql).fetch_all(pool).await?;
    ids.extend(rows.into_iter().map(|r| r.0));
    Ok(())
}

fn pick_survivor(rows: &[EntityDupRow]) -> Uuid {
    rows.iter()
        .max_by(|a, b| {
            let a_lotd = a
                .canonical_name
                .as_deref()
                .unwrap_or("")
                .to_ascii_lowercase()
                .contains("lotd");
            let b_lotd = b
                .canonical_name
                .as_deref()
                .unwrap_or("")
                .to_ascii_lowercase()
                .contains("lotd");
            b_lotd
                .cmp(&a_lotd)
                .then(a.event_count.cmp(&b.event_count))
                .then_with(|| {
                    let al = a.canonical_name.as_deref().unwrap_or("").len();
                    let bl = b.canonical_name.as_deref().unwrap_or("").len();
                    al.cmp(&bl)
                })
                .then(a.id.cmp(&b.id))
        })
        .map(|r| r.id)
        .expect("duplicate group is non-empty")
}

async fn merge_duplicate_qids(tx: &mut Transaction<'_, Postgres>) -> anyhow::Result<()> {
    let rows: Vec<EntityDupRow> = sqlx::query_as(
        "SELECT e.id, e.qid, e.canonical_name,
                (SELECT count(*) FROM canonical_events ce WHERE ce.entity_id = e.id) AS event_count
         FROM entities e
         WHERE e.qid IS NOT NULL
           AND e.qid IN (
               SELECT qid FROM entities WHERE qid IS NOT NULL GROUP BY qid HAVING count(*) > 1
           )",
    )
    .fetch_all(&mut **tx)
    .await?;

    let mut by_qid: HashMap<String, Vec<EntityDupRow>> = HashMap::new();
    for row in rows {
        by_qid.entry(row.qid.clone()).or_default().push(row);
    }

    for (_qid, group) in by_qid {
        let keep = pick_survivor(&group);
        for loser in group.iter().filter(|r| r.id != keep) {
            remap_entity_fks(tx, keep, loser.id).await?;
            sqlx::query("DELETE FROM entities WHERE id = $1")
                .bind(loser.id)
                .execute(&mut **tx)
                .await?;
        }
    }
    Ok(())
}

async fn table_exists(tx: &mut Transaction<'_, Postgres>, name: &str) -> anyhow::Result<bool> {
    let found: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM information_schema.tables
         WHERE table_schema = 'public' AND table_name = $1",
    )
    .bind(name)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(found.is_some())
}

async fn remap_entity_fks(
    tx: &mut Transaction<'_, Postgres>,
    keep: Uuid,
    lose: Uuid,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE canonical_events SET entity_id = $1 WHERE entity_id = $2")
        .bind(keep)
        .bind(lose)
        .execute(&mut **tx)
        .await
        .ok();
    sqlx::query("UPDATE event_candidates SET subject_entity_id = $1 WHERE subject_entity_id = $2")
        .bind(keep)
        .bind(lose)
        .execute(&mut **tx)
        .await
        .ok();

    if table_exists(tx, "entity_aliases").await? {
        sqlx::query(
            "UPDATE entity_aliases a SET entity_id = $1
             WHERE entity_id = $2
               AND NOT EXISTS (
                   SELECT 1 FROM entity_aliases k
                   WHERE k.entity_id = $1 AND k.surface = a.surface AND k.language = a.language
               )",
        )
        .bind(keep)
        .bind(lose)
        .execute(&mut **tx)
        .await?;
        sqlx::query("DELETE FROM entity_aliases WHERE entity_id = $1")
            .bind(lose)
            .execute(&mut **tx)
            .await?;
    }

    if table_exists(tx, "soft_claims").await? {
        sqlx::query("UPDATE soft_claims SET entity_id = $1 WHERE entity_id = $2")
            .bind(keep)
            .bind(lose)
            .execute(&mut **tx)
            .await?;
    }

    if table_exists(tx, "entity_periods").await? {
        sqlx::query(
            "UPDATE entity_periods p SET entity_id = $1
             WHERE entity_id = $2
               AND NOT EXISTS (
                   SELECT 1 FROM entity_periods k
                   WHERE k.entity_id = $1 AND k.period_id = p.period_id
               )",
        )
        .bind(keep)
        .bind(lose)
        .execute(&mut **tx)
        .await?;
        sqlx::query("DELETE FROM entity_periods WHERE entity_id = $1")
            .bind(lose)
            .execute(&mut **tx)
            .await?;
    }

    if table_exists(tx, "entity_profiles").await? {
        sqlx::query(
            "UPDATE entity_profiles p SET entity_id = $1
             WHERE entity_id = $2
               AND NOT EXISTS (
                   SELECT 1 FROM entity_profiles k
                   WHERE k.entity_id = $1 AND k.profile_slug = p.profile_slug AND k.kind = p.kind
               )",
        )
        .bind(keep)
        .bind(lose)
        .execute(&mut **tx)
        .await?;
        sqlx::query("DELETE FROM entity_profiles WHERE entity_id = $1")
            .bind(lose)
            .execute(&mut **tx)
            .await?;
    }

    let fks: Vec<(String, String)> = sqlx::query_as(
        "SELECT kcu.table_name, kcu.column_name
         FROM information_schema.table_constraints tc
         JOIN information_schema.key_column_usage kcu
           ON tc.constraint_name = kcu.constraint_name
          AND tc.table_schema = kcu.table_schema
         JOIN information_schema.constraint_column_usage ccu
           ON ccu.constraint_name = tc.constraint_name
          AND ccu.table_schema = tc.table_schema
         WHERE tc.constraint_type = 'FOREIGN KEY'
           AND tc.table_schema = 'public'
           AND ccu.table_name = 'entities'
           AND ccu.column_name = 'id'",
    )
    .fetch_all(&mut **tx)
    .await?;

    for (table, column) in fks {
        if matches!(
            table.as_str(),
            "entity_aliases" | "entity_periods" | "entity_profiles" | "soft_claims"
        ) {
            continue;
        }
        let sql = format!("UPDATE {table} SET {column} = $1 WHERE {column} = $2");
        let _ = sqlx::query(&sql)
            .bind(keep)
            .bind(lose)
            .execute(&mut **tx)
            .await;
    }
    Ok(())
}

async fn detach_canonical_event_fks(tx: &mut Transaction<'_, Postgres>) -> anyhow::Result<()> {
    if table_exists(tx, "soft_claims").await? {
        sqlx::query(
            "UPDATE soft_claims SET canonical_event_id = NULL WHERE canonical_event_id IS NOT NULL",
        )
        .execute(&mut **tx)
        .await?;
    }
    if table_exists(tx, "quality_claims").await? {
        sqlx::query(
            "UPDATE quality_claims SET canonical_event_id = NULL WHERE canonical_event_id IS NOT NULL",
        )
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query(
        "UPDATE event_candidates SET canonical_event_id = NULL WHERE canonical_event_id IS NOT NULL",
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Finish indexes/constraints 027 skipped when live data had collisions.
async fn ensure_deferred_pipeline_constraints(
    tx: &mut Transaction<'_, Postgres>,
) -> anyhow::Result<()> {
    sqlx::query("ALTER TABLE canonical_events DROP CONSTRAINT IF EXISTS canonical_events_pipeline_check")
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        r#"
        ALTER TABLE canonical_events
            ADD CONSTRAINT canonical_events_pipeline_check
            CHECK (pipeline IN ('legacy', 'person')) NOT VALID
        "#,
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query("ALTER TABLE canonical_events ALTER COLUMN pipeline SET DEFAULT 'person'")
        .execute(&mut **tx)
        .await?;
    sqlx::query("ALTER TABLE canonical_events VALIDATE CONSTRAINT canonical_events_pipeline_check")
        .execute(&mut **tx)
        .await?;

    sqlx::query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS uq_canonical_active_occurrence
            ON canonical_events (entity_id, occurrence_key)
            WHERE is_active AND pipeline = 'person' AND occurrence_key IS NOT NULL
        "#,
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS uq_canonical_active_singleton_birth_death
            ON canonical_events (entity_id, event_type)
            WHERE is_active
              AND pipeline = 'person'
              AND event_type IN ('birth', 'death')
        "#,
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS uq_canonical_events_active_fingerprint
            ON canonical_events (fingerprint)
            WHERE is_active AND fingerprint IS NOT NULL AND pipeline = 'person'
        "#,
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        r#"
        DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1 FROM pg_constraint WHERE conname = 'uq_event_evidence_dedup'
            ) THEN
                ALTER TABLE event_evidence
                    ADD CONSTRAINT uq_event_evidence_dedup
                    UNIQUE NULLS NOT DISTINCT (canonical_event_id, raw_document_id, evidence_hash);
            END IF;
        END $$
        "#,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn null_event_candidate_fks(tx: &mut Transaction<'_, Postgres>) -> anyhow::Result<()> {
    if table_exists(tx, "quality_claims").await? {
        sqlx::query("UPDATE quality_claims SET event_candidate_id = NULL WHERE event_candidate_id IS NOT NULL")
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: Uuid, name: &str, events: i64) -> EntityDupRow {
        EntityDupRow {
            id,
            qid: "Q1".into(),
            canonical_name: Some(name.into()),
            event_count: events,
        }
    }

    #[test]
    fn survivor_prefers_non_lotd_then_events_then_longest_name() {
        let lotd = row(Uuid::from_u128(1), "Napoleon LotD abc", 99);
        let short = row(Uuid::from_u128(2), "Nap", 3);
        let long = row(Uuid::from_u128(3), "Napoleon Bonaparte", 3);
        assert_eq!(pick_survivor(&[lotd, short, long]), Uuid::from_u128(3));
    }
}
