// crates/talaria-store/src/merimee_audit.rs
//! Read-only Mérimée pipeline metrics for operator validation.

use crate::merimee_csv::{stream_csv, RowReject};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::io::Read;

#[derive(Debug, Clone, Serialize)]
pub struct MerimeeAuditSnapshot {
    pub notices_total: i64,
    pub person_link_rows: i64,
    pub distinct_person_notices: i64,
    pub notices_with_coordinates: i64,
    pub notices_with_structured_persons: i64,
    pub notices_with_wikidata_qid: i64,
    pub notices_heritage_site_linked: i64,
    pub published_heritage_sites: i64,
    pub withheld_heritage_sites: i64,
    pub entity_heritage_links: i64,
    pub merimee_entity_matches: i64,
    pub import_runs: BTreeMap<String, i64>,
    pub last_completed_run: Option<ImportRunSummary>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ImportRunSummary {
    pub id: uuid::Uuid,
    pub status: String,
    pub rows_seen: i64,
    pub rows_upserted: i64,
    pub rows_unchanged: i64,
    pub rows_rejected: i64,
    pub last_row_number: i64,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CsvParseDryRun {
    pub rows_accepted: u64,
    pub rows_rejected: u64,
    pub reject_reasons: BTreeMap<String, u64>,
}

pub async fn collect_audit_snapshot(pool: &PgPool) -> anyhow::Result<MerimeeAuditSnapshot> {
    let notices_total: i64 = sqlx::query_scalar("SELECT count(*)::bigint FROM merimee_notices")
        .fetch_one(pool)
        .await?;
    let person_link_rows: i64 =
        sqlx::query_scalar("SELECT count(*)::bigint FROM merimee_notice_persons")
            .fetch_one(pool)
            .await?;
    let distinct_person_notices: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT notice_reference)::bigint FROM merimee_notice_persons",
    )
    .fetch_one(pool)
    .await?;
    let notices_with_coordinates: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM merimee_notices WHERE lat IS NOT NULL AND lon IS NOT NULL",
    )
    .fetch_one(pool)
    .await?;
    let notices_with_structured_persons: i64 = sqlx::query_scalar(
        r#"
        SELECT count(DISTINCT notice_reference)::bigint
        FROM merimee_notice_persons
        "#,
    )
    .fetch_one(pool)
    .await?;
    let notices_with_wikidata_qid: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM merimee_notices WHERE place_wikidata_qid IS NOT NULL",
    )
    .fetch_one(pool)
    .await?;
    let notices_heritage_site_linked: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM merimee_notices WHERE heritage_site_id IS NOT NULL",
    )
    .fetch_one(pool)
    .await?;
    let published_heritage_sites: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM heritage_sites WHERE publication_status = 'published'",
    )
    .fetch_one(pool)
    .await?;
    let withheld_heritage_sites: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM heritage_sites WHERE publication_status = 'withheld'",
    )
    .fetch_one(pool)
    .await?;
    let entity_heritage_links: i64 =
        sqlx::query_scalar("SELECT count(*)::bigint FROM entity_heritage_links")
            .fetch_one(pool)
            .await?;
    let merimee_entity_matches: i64 =
        sqlx::query_scalar("SELECT count(*)::bigint FROM merimee_entity_matches")
            .fetch_one(pool)
            .await?;

    let run_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, count(*)::bigint FROM merimee_import_runs GROUP BY status ORDER BY status",
    )
    .fetch_all(pool)
    .await?;
    let import_runs = run_rows.into_iter().collect();

    let last_completed_run = sqlx::query_as::<_, ImportRunSummary>(
        r#"
        SELECT id, status, rows_seen, rows_upserted, rows_unchanged, rows_rejected,
               last_row_number, finished_at
        FROM merimee_import_runs
        WHERE finished_at IS NOT NULL
        ORDER BY finished_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;

    Ok(MerimeeAuditSnapshot {
        notices_total,
        person_link_rows,
        distinct_person_notices,
        notices_with_coordinates,
        notices_with_structured_persons,
        notices_with_wikidata_qid,
        notices_heritage_site_linked,
        published_heritage_sites,
        withheld_heritage_sites,
        entity_heritage_links,
        merimee_entity_matches,
        import_runs,
        last_completed_run,
    })
}

pub fn dry_run_csv<R: Read>(reader: R) -> Result<CsvParseDryRun, csv::Error> {
    let mut report = CsvParseDryRun::default();
    stream_csv(
        reader,
        |_| report.rows_accepted += 1,
        |_, reason| {
            report.rows_rejected += 1;
            let key = reject_label(&reason);
            *report.reject_reasons.entry(key).or_insert(0) += 1;
        },
    )?;
    Ok(report)
}

fn reject_label(reason: &RowReject) -> String {
    match reason {
        RowReject::MissingReference => "missing_reference".into(),
        RowReject::InvalidReference => "invalid_reference".into(),
        RowReject::MissingTitle => "missing_title".into(),
        RowReject::InvalidCoordinates => "invalid_coordinates".into(),
        RowReject::MalformedRow(_) => "malformed_row".into(),
    }
}

pub async fn resume_row_from_last_partial(pool: &PgPool, source_uri: &str) -> anyhow::Result<u64> {
    let row: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT max(last_row_number)
        FROM merimee_import_runs
        WHERE status IN ('partial', 'failed')
          AND source_uri = $1
          AND last_row_number > 0
        "#,
    )
    .bind(source_uri)
    .fetch_optional(pool)
    .await?;
    Ok(row.unwrap_or(0).max(0) as u64)
}
