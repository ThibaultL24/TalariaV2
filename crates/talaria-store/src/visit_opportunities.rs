// crates/talaria-store/src/visit_opportunities.rs
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct VisitOpportunityRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub kind: String,
    pub title: String,
    pub summary: Option<String>,
    pub venue_label: Option<String>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub canonical_url: Option<String>,
    pub source_kind: String,
    pub source_record_id: String,
    pub fetched_at: DateTime<Utc>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct VisitOpportunityInsert {
    pub entity_id: Uuid,
    pub kind: String,
    pub title: String,
    pub summary: Option<String>,
    pub venue_label: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub canonical_url: Option<String>,
    pub source_kind: String,
    pub source_record_id: String,
}

pub async fn upsert_visit_opportunity(
    pool: &PgPool,
    row: &VisitOpportunityInsert,
) -> anyhow::Result<Uuid> {
    let id: Uuid = if row.lat.is_some() && row.lon.is_some() {
        sqlx::query_scalar(
            r#"
            INSERT INTO visit_opportunities (
                entity_id, kind, title, summary, venue_label, geom,
                starts_at, ends_at, canonical_url, source_kind, source_record_id, fetched_at
            )
            VALUES (
                $1, $2, $3, $4, $5,
                ST_SetSRID(ST_MakePoint($6, $7), 4326)::geography,
                $8, $9, $10, $11, $12, NOW()
            )
            ON CONFLICT (entity_id, source_kind, source_record_id) DO UPDATE SET
                kind = EXCLUDED.kind,
                title = EXCLUDED.title,
                summary = EXCLUDED.summary,
                venue_label = EXCLUDED.venue_label,
                geom = EXCLUDED.geom,
                starts_at = EXCLUDED.starts_at,
                ends_at = EXCLUDED.ends_at,
                canonical_url = EXCLUDED.canonical_url,
                fetched_at = NOW()
            RETURNING id
            "#,
        )
        .bind(row.entity_id)
        .bind(&row.kind)
        .bind(&row.title)
        .bind(&row.summary)
        .bind(&row.venue_label)
        .bind(row.lon)
        .bind(row.lat)
        .bind(row.starts_at)
        .bind(row.ends_at)
        .bind(&row.canonical_url)
        .bind(&row.source_kind)
        .bind(&row.source_record_id)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_scalar(
            r#"
            INSERT INTO visit_opportunities (
                entity_id, kind, title, summary, venue_label,
                starts_at, ends_at, canonical_url, source_kind, source_record_id, fetched_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW())
            ON CONFLICT (entity_id, source_kind, source_record_id) DO UPDATE SET
                kind = EXCLUDED.kind,
                title = EXCLUDED.title,
                summary = EXCLUDED.summary,
                venue_label = EXCLUDED.venue_label,
                starts_at = EXCLUDED.starts_at,
                ends_at = EXCLUDED.ends_at,
                canonical_url = EXCLUDED.canonical_url,
                fetched_at = NOW()
            RETURNING id
            "#,
        )
        .bind(row.entity_id)
        .bind(&row.kind)
        .bind(&row.title)
        .bind(&row.summary)
        .bind(&row.venue_label)
        .bind(row.starts_at)
        .bind(row.ends_at)
        .bind(&row.canonical_url)
        .bind(&row.source_kind)
        .bind(&row.source_record_id)
        .fetch_one(pool)
        .await?
    };
    Ok(id)
}

pub async fn list_visit_now(
    pool: &PgPool,
    entity_id: Uuid,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
    radius_meters: f64,
    limit: i64,
    bbox: Option<[f64; 4]>,
    map_only: bool,
) -> anyhow::Result<Vec<VisitOpportunityRow>> {
    let radius = radius_meters.max(100.0).min(500_000.0);
    let anchor_filter = r#"
          AND (
            vo.geom IS NULL
            OR EXISTS (
              SELECT 1 FROM canonical_events ce
              WHERE ce.entity_id = $1
                AND ce.is_active AND ce.pipeline = 'person'
                AND ce.map_eligible AND ce.geom IS NOT NULL
                AND (ce.timeline_eligible OR ce.visit_eligible)
                AND ST_DWithin(vo.geom::geography, ce.geom::geography, $4)
            )
          )
    "#;
    let base_select = r#"
        SELECT vo.id, vo.entity_id, vo.kind, vo.title, vo.summary, vo.venue_label,
               vo.starts_at, vo.ends_at, vo.canonical_url, vo.source_kind, vo.source_record_id,
               vo.fetched_at,
               ST_Y(vo.geom::geometry) AS lat, ST_X(vo.geom::geometry) AS lon
        FROM visit_opportunities vo
        WHERE vo.entity_id = $1
          AND (vo.ends_at IS NULL OR vo.ends_at >= $2)
          AND (vo.starts_at IS NULL OR vo.starts_at <= $3)
    "#;
    let map_clause = if map_only {
        " AND vo.geom IS NOT NULL"
    } else {
        ""
    };

    if let Some([west, south, east, north]) = bbox {
        let sql = format!(
            "{base_select}{map_clause}{anchor_filter}
             AND vo.geom IS NOT NULL
             AND ST_Intersects(vo.geom, ST_MakeEnvelope($5, $6, $7, $8, 4326)::geography)
             ORDER BY vo.starts_at ASC NULLS LAST, vo.title ASC LIMIT $9"
        );
        Ok(sqlx::query_as::<_, VisitOpportunityRow>(&sql)
            .bind(entity_id)
            .bind(window_start)
            .bind(window_end)
            .bind(radius)
            .bind(west)
            .bind(south)
            .bind(east)
            .bind(north)
            .bind(limit)
            .fetch_all(pool)
            .await?)
    } else {
        let sql = format!(
            "{base_select}{map_clause}{anchor_filter}
             ORDER BY vo.starts_at ASC NULLS LAST, vo.title ASC LIMIT $5"
        );
        Ok(sqlx::query_as::<_, VisitOpportunityRow>(&sql)
            .bind(entity_id)
            .bind(window_start)
            .bind(window_end)
            .bind(radius)
            .bind(limit)
            .fetch_all(pool)
            .await?)
    }
}
