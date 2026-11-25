// crates/talaria-store/src/poreia.rs
use chrono::{DateTime, NaiveDate, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoreiaDecision {
    Publish,
    Reject,
    Hold,
}

impl PoreiaDecision {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "publish" => Some(Self::Publish),
            "reject" => Some(Self::Reject),
            "hold" => Some(Self::Hold),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Publish => "publish",
            Self::Reject => "reject",
            Self::Hold => "hold",
        }
    }

    fn next_status(self) -> Option<&'static str> {
        match self {
            Self::Publish => Some("published"),
            Self::Reject => Some("rejected"),
            Self::Hold => None,
        }
    }
}

pub fn valid_reason_code(code: &str) -> bool {
    matches!(
        code,
        "location_verified"
            | "source_verified"
            | "relation_verified"
            | "license_verified"
            | "insufficient_source"
            | "not_the_subject"
    )
}

pub fn evidence_supports(site_qid: &str, evidence_url: &str, known_sources: &[String]) -> bool {
    let wikidata_page = format!("https://www.wikidata.org/wiki/{site_qid}");
    evidence_url == wikidata_page || known_sources.iter().any(|source| source == evidence_url)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PoreiaSiteRow {
    pub id: Uuid,
    pub wikidata_qid: String,
    pub label: String,
    pub kind: String,
    pub visitability: String,
    pub lat: f64,
    pub lon: f64,
    pub opening_hours: Option<String>,
    pub hours_checked_at: Option<DateTime<Utc>>,
    pub narratives: Value,
    pub media: Value,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PoreiaOccurrenceRow {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub summary: Option<String>,
    pub venue_label: Option<String>,
    pub occurs_on: Option<NaiveDate>,
    pub ends_on: Option<NaiveDate>,
    pub time_precision: String,
    pub source_url: String,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

pub async fn list_published_sites(
    pool: &PgPool,
    lon: f64,
    lat: f64,
    radius_m: f64,
    limit: i64,
    offset: i64,
    bbox: Option<(f64, f64, f64, f64)>,
) -> Result<Vec<PoreiaSiteRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT s.id,
               s.wikidata_qid,
               s.label,
               s.kind,
               s.visitability,
               ST_Y(s.geom::geometry) AS lat,
               ST_X(s.geom::geometry) AS lon,
               s.osm_opening_hours AS opening_hours,
               s.osm_checked_at AS hours_checked_at,
               COALESCE((
                   SELECT json_agg(json_build_object(
                       'relation_role', n.relation_role,
                       'relation_pid', n.relation_pid,
                       'subject_qid', n.subject_qid,
                       'summary', n.summary,
                       'source_url', n.source_url
                   ) ORDER BY n.relation_role)
                   FROM visit_site_narratives n
                   WHERE n.site_id = s.id AND n.publication_status = 'published'
               ), '[]'::json) AS narratives,
               COALESCE((
                   SELECT json_agg(json_build_object(
                       'author', m.author,
                       'license', m.license,
                       'attribution', m.attribution,
                       'source_url', m.source_url
                   ) ORDER BY m.source_url)
                   FROM visit_site_media m
                   WHERE m.site_id = s.id
                     AND m.publication_status = 'published'
                     AND m.author IS NOT NULL
                     AND m.license IS NOT NULL
                     AND m.attribution IS NOT NULL
               ), '[]'::json) AS media
        FROM visit_sites s
        WHERE s.publication_status = 'published'
          AND ST_DWithin(
                s.geom,
                ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography,
                $3
              )
          AND (
                $6::float8 IS NULL
                OR ST_Intersects(
                     s.geom,
                     ST_MakeEnvelope($6, $7, $8, $9, 4326)::geography
                   )
              )
        ORDER BY s.label ASC, s.id ASC
        LIMIT $4 OFFSET $5
        "#,
    )
    .bind(lon)
    .bind(lat)
    .bind(radius_m)
    .bind(limit)
    .bind(offset)
    .bind(bbox.map(|value| value.0))
    .bind(bbox.map(|value| value.1))
    .bind(bbox.map(|value| value.2))
    .bind(bbox.map(|value| value.3))
    .fetch_all(pool)
    .await
}

pub async fn list_published_occurrences(
    pool: &PgPool,
    lon: f64,
    lat: f64,
    radius_m: f64,
    from: NaiveDate,
    to: NaiveDate,
    limit: i64,
    offset: i64,
    bbox: Option<(f64, f64, f64, f64)>,
) -> Result<Vec<PoreiaOccurrenceRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT id, kind, title, summary, venue_label, occurs_on, ends_on,
               time_precision, source_url,
               ST_Y(geom::geometry) AS lat,
               ST_X(geom::geometry) AS lon
        FROM poreia_occurrences
        WHERE publication_status = 'published'
          AND geom IS NOT NULL
          AND ST_DWithin(
                geom,
                ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography,
                $3
              )
          AND (ends_on IS NULL OR ends_on >= $4)
          AND (occurs_on IS NULL OR occurs_on <= $5)
          AND (
                $8::float8 IS NULL
                OR ST_Intersects(
                     geom,
                     ST_MakeEnvelope($8, $9, $10, $11, 4326)::geography
                   )
              )
        ORDER BY occurs_on ASC NULLS LAST, id ASC
        LIMIT $6 OFFSET $7
        "#,
    )
    .bind(lon)
    .bind(lat)
    .bind(radius_m)
    .bind(from)
    .bind(to)
    .bind(limit)
    .bind(offset)
    .bind(bbox.map(|value| value.0))
    .bind(bbox.map(|value| value.1))
    .bind(bbox.map(|value| value.2))
    .bind(bbox.map(|value| value.3))
    .fetch_all(pool)
    .await
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReviewOutcome {
    MissingSite,
    EvidenceRejected,
    Stored { publication_status: String },
}

pub async fn review_poreia_site(
    pool: &PgPool,
    site_id: Uuid,
    actor_id: Uuid,
    decision: PoreiaDecision,
    reason_code: &str,
    evidence_url: &str,
) -> Result<ReviewOutcome, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let site: Option<(String, String)> = sqlx::query_as(
        "SELECT wikidata_qid, publication_status FROM visit_sites WHERE id = $1 FOR UPDATE",
    )
    .bind(site_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((qid, current_status)) = site else {
        return Ok(ReviewOutcome::MissingSite);
    };
    let known_sources = source_urls_for_site(&mut tx, site_id).await?;
    if decision == PoreiaDecision::Publish && !evidence_supports(&qid, evidence_url, &known_sources)
    {
        return Ok(ReviewOutcome::EvidenceRejected);
    }
    sqlx::query(
        r#"
        INSERT INTO visit_site_reviews
            (site_id, wikidata_qid, decision, reason_code, evidence_url, actor_id)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(site_id)
    .bind(&qid)
    .bind(decision.as_str())
    .bind(reason_code)
    .bind(evidence_url)
    .bind(actor_id)
    .execute(&mut *tx)
    .await?;
    let stored_status = if let Some(status) = decision.next_status() {
        sqlx::query("SELECT set_config('talaria.poreia_review', 'on', true)")
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE visit_sites SET publication_status = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(site_id)
        .bind(status)
        .execute(&mut *tx)
        .await?;
        if decision == PoreiaDecision::Publish {
            publish_matching_evidence(&mut tx, site_id, evidence_url).await?;
        }
        status.to_string()
    } else {
        current_status
    };
    tx.commit().await?;
    Ok(ReviewOutcome::Stored {
        publication_status: stored_status,
    })
}

async fn source_urls_for_site(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    site_id: Uuid,
) -> Result<Vec<String>, sqlx::Error> {
    let mut urls = Vec::new();
    let narratives: Vec<(String,)> =
        sqlx::query_as("SELECT source_url FROM visit_site_narratives WHERE site_id = $1")
            .bind(site_id)
            .fetch_all(&mut **tx)
            .await?;
    let media: Vec<(String,)> =
        sqlx::query_as("SELECT source_url FROM visit_site_media WHERE site_id = $1")
            .bind(site_id)
            .fetch_all(&mut **tx)
            .await?;
    let occurrences: Vec<(String,)> =
        sqlx::query_as("SELECT source_url FROM poreia_occurrences WHERE site_id = $1")
            .bind(site_id)
            .fetch_all(&mut **tx)
            .await?;
    urls.extend(narratives.into_iter().map(|row| row.0));
    urls.extend(media.into_iter().map(|row| row.0));
    urls.extend(occurrences.into_iter().map(|row| row.0));
    Ok(urls)
}

async fn publish_matching_evidence(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    site_id: Uuid,
    evidence_url: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE visit_site_narratives
        SET publication_status = 'published'
        WHERE site_id = $1 AND source_url = $2 AND publication_status = 'quarantine'
        "#,
    )
    .bind(site_id)
    .bind(evidence_url)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        r#"
        UPDATE visit_site_media
        SET publication_status = 'published'
        WHERE site_id = $1
          AND source_url = $2
          AND publication_status = 'quarantine'
          AND author IS NOT NULL
          AND license IS NOT NULL
          AND attribution IS NOT NULL
        "#,
    )
    .bind(site_id)
    .bind(evidence_url)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        r#"
        UPDATE poreia_occurrences
        SET publication_status = 'published'
        WHERE site_id = $1 AND source_url = $2 AND publication_status = 'quarantine'
        "#,
    )
    .bind(site_id)
    .bind(evidence_url)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_must_name_the_site_or_a_stored_source() {
        let known = vec!["https://www.wikidata.org/wiki/Q64436".to_string()];
        assert!(evidence_supports(
            "Q64436",
            "https://www.wikidata.org/wiki/Q64436",
            &known
        ));
        assert!(!evidence_supports(
            "Q64436",
            "https://www.wikidata.org/wiki/Q517",
            &[]
        ));
        assert!(!evidence_supports(
            "Q64436",
            "http://www.wikidata.org/wiki/Q64436",
            &[]
        ));
    }

    async fn insert_site(
        pool: &PgPool,
        qid: &str,
        label: &str,
        lon: f64,
        lat: f64,
        status: &str,
    ) -> Uuid {
        sqlx::query_scalar(
            r#"
            INSERT INTO visit_sites (wikidata_qid, label, kind, geom, publication_status, retrieved_at)
            VALUES ($1, $2, 'monument', ST_SetSRID(ST_MakePoint($3, $4), 4326)::geography, $5, NOW())
            RETURNING id
            "#,
        )
        .bind(qid)
        .bind(label)
        .bind(lon)
        .bind(lat)
        .bind(status)
        .fetch_one(pool)
        .await
        .expect("insert site")
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn published_sites_stay_inside_the_radius_and_skip_quarantine(pool: PgPool) {
        let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM canonical_events")
            .fetch_one(&pool)
            .await
            .unwrap();
        insert_site(
            &pool,
            "Q900001",
            "Published near",
            2.3522,
            48.8566,
            "published",
        )
        .await;
        insert_site(
            &pool,
            "Q900002",
            "Quarantine near",
            2.3523,
            48.8567,
            "quarantine",
        )
        .await;
        insert_site(
            &pool,
            "Q900003",
            "Published far",
            4.6211,
            43.6569,
            "published",
        )
        .await;
        let rows = list_published_sites(&pool, 2.3522, 48.8566, 10_000.0, 50, 0, None)
            .await
            .unwrap();
        let labels: Vec<_> = rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(labels, ["Published near"]);
        let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM canonical_events")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(before, after);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn review_is_append_only_and_blocks_unreviewed_status_edits(pool: PgPool) {
        let site_id = insert_site(
            &pool,
            "Q64436",
            "Arc de Triomphe",
            2.29504,
            48.87378,
            "quarantine",
        )
        .await;
        let actor: Uuid =
            sqlx::query_scalar("INSERT INTO users (display_name) VALUES ('operator') RETURNING id")
                .fetch_one(&pool)
                .await
                .unwrap();
        let rejected = review_poreia_site(
            &pool,
            site_id,
            actor,
            PoreiaDecision::Publish,
            "location_verified",
            "https://example.com/not-the-source",
        )
        .await
        .unwrap();
        assert_eq!(rejected, ReviewOutcome::EvidenceRejected);
        let published = review_poreia_site(
            &pool,
            site_id,
            actor,
            PoreiaDecision::Publish,
            "location_verified",
            "https://www.wikidata.org/wiki/Q64436",
        )
        .await
        .unwrap();
        assert_eq!(
            published,
            ReviewOutcome::Stored {
                publication_status: "published".to_string()
            }
        );
        let direct =
            sqlx::query("UPDATE visit_sites SET publication_status = 'quarantine' WHERE id = $1")
                .bind(site_id)
                .execute(&pool)
                .await;
        assert!(direct
            .unwrap_err()
            .to_string()
            .contains("poreia_publication_requires_review"));
        let rewrite =
            sqlx::query("UPDATE visit_site_reviews SET decision = 'hold' WHERE site_id = $1")
                .bind(site_id)
                .execute(&pool)
                .await;
        assert!(rewrite
            .unwrap_err()
            .to_string()
            .contains("visit_site_reviews_append_only"));
        let visible = list_published_sites(&pool, 2.29504, 48.87378, 2_000.0, 10, 0, None)
            .await
            .unwrap();
        assert_eq!(visible.len(), 1);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn day_precision_occurrences_hide_quarantine(pool: PgPool) {
        let site_id = insert_site(
            &pool,
            "Q17076145",
            "Fondation Vincent van Gogh Arles",
            4.625651,
            43.678131,
            "published",
        )
        .await;
        sqlx::query(
            r#"
            INSERT INTO poreia_occurrences (
                site_id, kind, title, occurs_on, ends_on, time_precision,
                source_url, source_record_id, publication_status, geom
            )
            VALUES
              ($1, 'exhibition', 'Visible', '2024-06-01', '2024-09-08', 'day',
               'https://www.wikidata.org/wiki/Q141613312', 'visible', 'published',
               ST_SetSRID(ST_MakePoint(4.625651, 43.678131), 4326)::geography),
              ($1, 'exhibition', 'Hidden', '2024-06-01', '2024-09-08', 'day',
               'https://www.wikidata.org/wiki/Q60054302', 'hidden', 'quarantine',
               ST_SetSRID(ST_MakePoint(4.625651, 43.678131), 4326)::geography)
            "#,
        )
        .bind(site_id)
        .execute(&pool)
        .await
        .unwrap();
        let from = NaiveDate::from_ymd_opt(2024, 6, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2024, 9, 8).unwrap();
        let rows =
            list_published_occurrences(&pool, 4.625651, 43.678131, 5_000.0, from, to, 30, 0, None)
                .await
                .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "Visible");
        assert_eq!(rows[0].time_precision, "day");
        assert_eq!(rows[0].occurs_on, Some(from));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn pilot_import_is_idempotent_and_stays_in_quarantine(pool: PgPool) {
        let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM canonical_events")
            .fetch_one(&pool)
            .await
            .unwrap();
        let sql = include_str!("../../../scripts/poreia/pilot_stage.sql");
        sqlx::raw_sql(sql)
            .execute(&pool)
            .await
            .expect("first pilot import");
        sqlx::raw_sql(sql)
            .execute(&pool)
            .await
            .expect("second pilot import");
        let quarantined: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM visit_sites
            WHERE wikidata_qid IN ('Q59210966', 'Q64436', 'Q315318', 'Q17076145')
              AND publication_status = 'quarantine'
            "#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(quarantined, 4);
        let published: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM visit_sites
            WHERE wikidata_qid IN ('Q59210966', 'Q64436', 'Q315318', 'Q17076145')
              AND publication_status = 'published'
            "#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(published, 0);
        let visible = list_published_sites(&pool, 2.3522, 48.8566, 20_000.0, 50, 0, None)
            .await
            .unwrap();
        assert!(visible.iter().all(|row| row.wikidata_qid != "Q64436"));
        let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM canonical_events")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(before, after);
    }
}
