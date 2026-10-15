// crates/talaria-store/src/merimee.rs
//! Persist and query local Mérimée notices.

use crate::merimee_csv::{person_tokens_for_query, provenance_for_row, stream_csv, ParsedMerimeeRow, RowReject};
use crate::visit_catalog::{MerimeeHit, person_named_in};
use serde_json::json;
use sqlx::PgPool;
use std::io::Read;
use std::path::Path;
use tokio::task;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct MerimeeImportReport {
    pub run_id: Uuid,
    pub rows_seen: u64,
    pub rows_upserted: u64,
    pub rows_unchanged: u64,
    pub rows_rejected: u64,
    pub entity_links_added: u64,
    pub heritage_links_added: u64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MerimeeNoticeSummary {
    pub reference: String,
    pub title: String,
    pub commune: Option<String>,
    pub protection_kind: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub place_wikidata_qid: Option<String>,
    pub heritage_site_id: Option<Uuid>,
}

pub async fn import_merimee_csv<R: Read + Send + 'static>(
    pool: &PgPool,
    source_uri: &str,
    file_checksum: Option<&str>,
    reader: R,
    resume_after_row: u64,
) -> anyhow::Result<MerimeeImportReport> {
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(746574601)")
        .fetch_one(pool)
        .await?;
    if !locked {
        anyhow::bail!("merimee_import_already_running");
    }

    let run_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO merimee_import_runs (source_uri, file_checksum, status)
        VALUES ($1, $2, 'running')
        RETURNING id
        "#,
    )
    .bind(source_uri)
    .bind(file_checksum)
    .fetch_one(pool)
    .await?;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<ParsedMerimeeRow>(64);
    let (reject_tx, mut reject_rx) = tokio::sync::mpsc::channel::<(u64, RowReject)>(64);
    let parse_task = task::spawn_blocking(move || {
        let _ = stream_csv(
            reader,
            |row| {
                if tx.blocking_send(row).is_err() {
                    tracing::warn!("merimee import channel closed");
                }
            },
            |row_number, reason| {
                let _ = reject_tx.blocking_send((row_number, reason));
            },
        );
    });

    let mut rows_seen = 0_u64;
    let mut rows_upserted = 0_u64;
    let mut rows_unchanged = 0_u64;
    let mut rows_rejected = 0_u64;
    let mut last_row = resume_after_row;
    let source_uri = source_uri.to_string();

    while let Some(row) = rx.recv().await {
        if row.row_number <= resume_after_row {
            continue;
        }
        rows_seen += 1;
        last_row = row.row_number;
        if rows_seen.is_multiple_of(5000) {
            tracing::info!(rows_seen, last_row, "merimee import progress");
            eprintln!("phase=import_progress rows_seen={rows_seen} last_row={last_row}");
        }
        match upsert_notice(pool, &source_uri, &row).await {
            Ok(UpsertOutcome::Inserted | UpsertOutcome::Updated) => rows_upserted += 1,
            Ok(UpsertOutcome::Unchanged) => rows_unchanged += 1,
            Err(err) => {
                rows_rejected += 1;
                tracing::warn!(reference = %row.reference, error = %err, "merimee row persist failed");
            }
        }
    }
    while let Some((row_number, _reason)) = reject_rx.recv().await {
        if row_number > resume_after_row {
            rows_rejected += 1;
        }
    }
    let _ = parse_task.await;

    let (entity_links, heritage_links) = reconcile_notices(pool).await?;

    let status = if rows_rejected > 0 && rows_upserted + rows_unchanged > 0 {
        "partial"
    } else if rows_rejected > 0 && rows_upserted + rows_unchanged == 0 {
        "failed"
    } else {
        "completed"
    };

    sqlx::query(
        r#"
        UPDATE merimee_import_runs
        SET finished_at = NOW(),
            status = $2,
            rows_seen = $3,
            rows_upserted = $4,
            rows_unchanged = $5,
            rows_rejected = $6,
            last_row_number = $7
        WHERE id = $1
        "#,
    )
    .bind(run_id)
    .bind(status)
    .bind(rows_seen as i64)
    .bind(rows_upserted as i64)
    .bind(rows_unchanged as i64)
    .bind(rows_rejected as i64)
    .bind(last_row as i64)
    .execute(pool)
    .await?;

    let _ = sqlx::query_scalar::<_, bool>("SELECT pg_advisory_unlock(746574601)")
        .fetch_one(pool)
        .await?;

    Ok(MerimeeImportReport {
        run_id,
        rows_seen,
        rows_upserted,
        rows_unchanged,
        rows_rejected,
        entity_links_added: entity_links,
        heritage_links_added: heritage_links,
    })
}

pub async fn import_merimee_file(
    pool: &PgPool,
    path: &Path,
    source_uri: &str,
    resume_after_row: u64,
) -> anyhow::Result<MerimeeImportReport> {
    let bytes = std::fs::read(path)?;
    let checksum = sha256_hex(&bytes);
    let reader = std::io::Cursor::new(bytes);
    import_merimee_csv(pool, source_uri, Some(&checksum), reader, resume_after_row).await
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

enum UpsertOutcome {
    Inserted,
    Updated,
    Unchanged,
}

async fn upsert_notice(pool: &PgPool, source_uri: &str, row: &ParsedMerimeeRow) -> anyhow::Result<UpsertOutcome> {
    let existing_hash: Option<String> = sqlx::query_scalar("SELECT content_hash FROM merimee_notices WHERE reference = $1")
        .bind(&row.reference)
        .fetch_optional(pool)
        .await?;
    if existing_hash.as_deref() == Some(&row.content_hash) {
        return Ok(UpsertOutcome::Unchanged);
    }
    let provenance = provenance_for_row(source_uri, row.row_number);
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"
        INSERT INTO merimee_notices (
            reference, title, commune, department, region, protection_kind, authors,
            historique, description, personalities_raw, external_links_raw, place_wikidata_qid,
            lat, lon, geom, content_hash, provenance_json, updated_at
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
            CASE WHEN $13::float8 IS NULL OR $14::float8 IS NULL THEN NULL
                 ELSE ST_SetSRID(ST_MakePoint($14, $13), 4326)::geography END,
            $15, $16, NOW()
        )
        ON CONFLICT (reference) DO UPDATE SET
            title = EXCLUDED.title,
            commune = EXCLUDED.commune,
            department = EXCLUDED.department,
            region = EXCLUDED.region,
            protection_kind = EXCLUDED.protection_kind,
            authors = EXCLUDED.authors,
            historique = EXCLUDED.historique,
            description = EXCLUDED.description,
            personalities_raw = EXCLUDED.personalities_raw,
            external_links_raw = EXCLUDED.external_links_raw,
            place_wikidata_qid = EXCLUDED.place_wikidata_qid,
            lat = EXCLUDED.lat,
            lon = EXCLUDED.lon,
            geom = EXCLUDED.geom,
            content_hash = EXCLUDED.content_hash,
            provenance_json = EXCLUDED.provenance_json,
            updated_at = NOW()
        "#,
    )
    .bind(&row.reference)
    .bind(&row.title)
    .bind(&row.commune)
    .bind(&row.department)
    .bind(&row.region)
    .bind(&row.protection_kind)
    .bind(&row.authors)
    .bind(&row.historique)
    .bind(&row.description)
    .bind(&row.personalities_raw)
    .bind(&row.external_links_raw)
    .bind(&row.place_wikidata_qid)
    .bind(row.lat)
    .bind(row.lon)
    .bind(&row.content_hash)
    .bind(provenance)
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM merimee_notice_persons WHERE notice_reference = $1")
        .bind(&row.reference)
        .execute(&mut *tx)
        .await?;

    for person in &row.persons {
        sqlx::query(
            r#"
            INSERT INTO merimee_notice_persons (
                notice_reference, person_name_raw, person_name_normalized, role_label
            )
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (notice_reference, person_name_raw) DO NOTHING
            "#,
        )
        .bind(&row.reference)
        .bind(&person.raw)
        .bind(&person.name_normalized)
        .bind(&person.role_label)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(if existing_hash.is_some() {
        UpsertOutcome::Updated
    } else {
        UpsertOutcome::Inserted
    })
}

async fn reconcile_notices(pool: &PgPool) -> anyhow::Result<(u64, u64)> {
    let heritage_updated = sqlx::query(
        r#"
        UPDATE merimee_notices mn
        SET heritage_site_id = hs.id
        FROM heritage_sites hs
        WHERE mn.place_wikidata_qid IS NOT NULL
          AND hs.place_qid = mn.place_wikidata_qid
          AND (mn.heritage_site_id IS DISTINCT FROM hs.id)
        "#,
    )
    .execute(pool)
    .await?
    .rows_affected();

    let heritage_by_ref = sqlx::query(
        r#"
        UPDATE merimee_notices mn
        SET heritage_site_id = hs.id
        FROM heritage_sites hs
        WHERE hs.external_ids->>'merimee' = mn.reference
          AND (mn.heritage_site_id IS DISTINCT FROM hs.id)
        "#,
    )
    .execute(pool)
    .await?
    .rows_affected();

    Ok((0, heritage_updated + heritage_by_ref))
}

pub async fn merimee_hits_for_entity(
    pool: &PgPool,
    person_label: &str,
    person_qid: Option<&str>,
) -> anyhow::Result<Vec<MerimeeHit>> {
    let mut references = Vec::new();
    if let Some(qid) = person_qid {
        let matched: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT mn.reference
            FROM merimee_entity_matches em
            JOIN merimee_notices mn ON mn.reference = em.notice_reference
            JOIN entities e ON e.id = em.entity_id
            WHERE e.qid = $1 AND em.confidence IN ('high', 'medium')
            "#,
        )
        .bind(qid)
        .fetch_all(pool)
        .await?;
        references.extend(matched);
    }

    let tokens = person_tokens_for_query(person_label);
    if !tokens.is_empty() {
        let mut query = String::from(
            "SELECT DISTINCT mp.notice_reference FROM merimee_notice_persons mp WHERE ",
        );
        for (index, token) in tokens.iter().enumerate() {
            if index > 0 {
                query.push_str(" AND ");
            }
            query.push_str(&format!("mp.person_name_normalized LIKE '%{}%'", escape_like(token)));
        }
        let person_refs: Vec<String> = sqlx::query_scalar(&query).fetch_all(pool).await?;
        references.extend(person_refs);
    }
    references.sort();
    references.dedup();

    let mut hits = Vec::new();
    for reference in references {
        let Some(notice) = fetch_notice_for_hit(pool, &reference).await? else {
            continue;
        };
        let personalities: Vec<String> = sqlx::query_scalar(
            "SELECT person_name_raw FROM merimee_notice_persons WHERE notice_reference = $1",
        )
        .bind(&reference)
        .fetch_all(pool)
        .await?;
        if !personalities.iter().any(|name| person_named_in(person_label, name)) {
            continue;
        }
        hits.push(MerimeeHit {
            ref_id: notice.reference,
            title: notice.title,
            hist: notice.historique,
            personalities,
            lat: notice.lat,
            lon: notice.lon,
            protection_kind: notice.protection_kind,
            commune: notice.commune,
            structured_roles: personality_roles(pool, &reference).await?,
        });
        if hits.len() >= 40 {
            break;
        }
    }
    Ok(hits)
}

async fn personality_roles(pool: &PgPool, reference: &str) -> anyhow::Result<Vec<(String, Option<String>)>> {
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT person_name_raw, role_label FROM merimee_notice_persons WHERE notice_reference = $1",
    )
    .bind(reference)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

async fn fetch_notice_for_hit(pool: &PgPool, reference: &str) -> anyhow::Result<Option<MerimeeNoticeRow>> {
    sqlx::query_as(
        r#"
        SELECT reference, title, commune, protection_kind, historique, lat, lon
        FROM merimee_notices WHERE reference = $1
        "#,
    )
    .bind(reference)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

#[derive(Debug, sqlx::FromRow)]
struct MerimeeNoticeRow {
    reference: String,
    title: String,
    commune: Option<String>,
    protection_kind: Option<String>,
    historique: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
}

fn escape_like(token: &str) -> String {
    token.replace('%', "").replace('_', "")
}

pub async fn get_merimee_notice(pool: &PgPool, reference: &str) -> anyhow::Result<Option<MerimeeNoticeSummary>> {
    sqlx::query_as(
        r#"
        SELECT reference, title, commune, protection_kind, lat, lon, place_wikidata_qid, heritage_site_id
        FROM merimee_notices WHERE reference = $1
        "#,
    )
    .bind(reference)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

pub async fn count_notices(pool: &PgPool) -> anyhow::Result<i64> {
    sqlx::query_scalar("SELECT count(*)::bigint FROM merimee_notices")
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merimee_csv::{notice_content_hash, MerimeeColumns, parse_csv_row};
    use csv::ReaderBuilder;

    #[sqlx::test(migrations = "../../migrations")]
    async fn merimee_import_idempotent_and_links_persons(pool: sqlx::PgPool) {
        let fixture = include_str!("../../../fixtures/merimee/sample.csv");
        let first = import_merimee_csv(
            &pool,
            "fixture://sample.csv",
            Some("test"),
            fixture.as_bytes(),
            0,
        )
        .await
        .unwrap();
        assert_eq!(first.rows_upserted, 3);
        assert_eq!(first.rows_rejected, 2);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM merimee_notices")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 3);
        let hugo_notices: i64 = sqlx::query_scalar(
            r#"
            SELECT count(DISTINCT mp.notice_reference)
            FROM merimee_notice_persons mp
            WHERE mp.person_name_normalized LIKE '%hugo%' AND mp.person_name_normalized LIKE '%victor%'
            "#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(hugo_notices, 3);

        let second = import_merimee_csv(
            &pool,
            "fixture://sample.csv",
            Some("test"),
            fixture.as_bytes(),
            0,
        )
        .await
        .unwrap();
        assert_eq!(second.rows_unchanged, 3);
        assert_eq!(second.rows_upserted, 0);

        let entity_id: Uuid = sqlx::query_scalar(
            "INSERT INTO entities (wikipedia_title, wiki_lang, canonical_name, qid) VALUES ('Victor Hugo', 'fr', 'Victor Hugo', 'Q535') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let hits = merimee_hits_for_entity(&pool, "Victor Hugo", Some("Q535")).await.unwrap();
        assert_eq!(hits.len(), 3);
        assert!(hits.iter().all(|hit| !hit.personalities.is_empty()));

        let before_events: i64 = sqlx::query_scalar("SELECT count(*) FROM canonical_events WHERE entity_id = $1")
            .bind(entity_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(before_events, 0);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn concurrent_imports_do_not_duplicate_notices(pool: sqlx::PgPool) {
        let fixture = include_str!("../../../fixtures/merimee/sample.csv");
        let pool_a = pool.clone();
        let pool_b = pool.clone();
        let reader_a = std::io::Cursor::new(fixture.as_bytes().to_vec());
        let reader_b = std::io::Cursor::new(fixture.as_bytes().to_vec());
        let (left, right) = tokio::join!(
            import_merimee_csv(&pool_a, "fixture://a", Some("a"), reader_a, 0),
            import_merimee_csv(&pool_b, "fixture://b", Some("b"), reader_b, 0),
        );
        left.unwrap();
        right.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM merimee_notices")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn content_hash_stable() {
        let a = notice_content_hash("PA1", "Maison", Some("Paris"), Some(48.8), Some(2.3), None, Some("arrêté"));
        let b = notice_content_hash("PA1", "Maison", Some("Paris"), Some(48.8), Some(2.3), None, Some("arrêté"));
        assert_eq!(a, b);
    }

    #[test]
    fn malformed_fixture_row_rejected() {
        let fixture = include_str!("../../../fixtures/merimee/sample.csv");
        let mut reader = ReaderBuilder::new().delimiter(b';').has_headers(true).from_reader(fixture.as_bytes());
        let columns = MerimeeColumns::from_headers(reader.headers().unwrap()).unwrap();
        let mut rejected = 0;
        for (index, record) in reader.records().enumerate() {
            let record = record.unwrap();
            if parse_csv_row(&columns, &record, index as u64 + 2).is_err() {
                rejected += 1;
            }
        }
        assert_eq!(rejected, 2);
    }
}
