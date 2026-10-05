// crates/talaria-store/src/visit.rs
use crate::canonical_events::CanonicalEventRow;
use sqlx::{PgPool, QueryBuilder, Postgres};
use uuid::Uuid;

pub const VISIT_COMMEMORATIVE_TYPES: &[&str] = &[
    "museum",
    "memorial",
    "statue",
    "street_naming",
    "commemoration",
];

pub fn eligibility_for_event_type(event_type: &str) -> (bool, bool) {
    let kind = event_type.trim().to_lowercase();
    if VISIT_COMMEMORATIVE_TYPES.contains(&kind.as_str()) {
        (false, true)
    } else {
        (true, false)
    }
}

pub async fn list_visit_heritage(
    pool: &PgPool,
    entity_id: Uuid,
    limit: i64,
    bbox: Option<[f64; 4]>,
    map_only: bool,
) -> anyhow::Result<Vec<CanonicalEventRow>> {
    let mut sql = QueryBuilder::<Postgres>::new(
        "SELECT ce.id, ce.entity_id, COALESCE(e.canonical_name, e.wikipedia_title) AS person_name, \
         ce.event_type, ce.epistemic_status, ce.title, ce.summary, ce.start_time, ce.time_json, \
         ce.place_label, ce.confidence, ce.map_eligible, \
         ST_Y(ce.geom::geometry) AS lat, ST_X(ce.geom::geometry) AS lon \
         FROM canonical_events ce \
         JOIN entities e ON e.id = ce.entity_id \
         WHERE ce.entity_id = ",
    );
    sql.push_bind(entity_id);
    sql.push(
        " AND ce.is_active AND ce.pipeline = 'person' AND ce.visit_eligible = true",
    );
    if map_only {
        sql.push(" AND ce.map_eligible AND ce.geom IS NOT NULL");
    }
    if let Some([west, south, east, north]) = bbox {
        sql.push(" AND ST_Intersects(ce.geom, ST_MakeEnvelope(")
            .push_bind(west)
            .push(",")
            .push_bind(south)
            .push(",")
            .push_bind(east)
            .push(",")
            .push_bind(north)
            .push(", 4326)::geography)");
    }
    sql.push(" ORDER BY ce.start_time ASC NULLS LAST, ce.title ASC LIMIT ")
        .push_bind(limit);
    Ok(sql
        .build_query_as::<CanonicalEventRow>()
        .fetch_all(pool)
        .await?)
}

/// Same commemorative type within `radius_m` — avoids stacked map pins from fixture + ingest.
pub async fn find_nearby_visit_heritage(
    pool: &PgPool,
    entity_id: Uuid,
    event_type: &str,
    lat: f64,
    lon: f64,
    radius_m: f64,
) -> anyhow::Result<Option<Uuid>> {
    if !radius_m.is_finite() || radius_m <= 0.0 {
        return Ok(None);
    }
    let id = sqlx::query_scalar(
        r#"
        SELECT id FROM canonical_events
        WHERE entity_id = $1
          AND pipeline = 'person'
          AND is_active
          AND visit_eligible
          AND event_type = $2
          AND geom IS NOT NULL
          AND ST_DWithin(
            geom,
            ST_SetSRID(ST_MakePoint($3, $4), 4326)::geography,
            $5
          )
        ORDER BY evidence_count DESC NULLS LAST, created_at ASC
        LIMIT 1
        "#,
    )
    .bind(entity_id)
    .bind(event_type)
    .bind(lon)
    .bind(lat)
    .bind(radius_m)
    .fetch_optional(pool)
    .await?;
    Ok(id)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct VisitHeritageAuditRow {
    pub id: Uuid,
    pub event_type: String,
    pub title: String,
    pub occurrence_key: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub evidence_count: i32,
}

pub async fn list_visit_heritage_for_audit(
    pool: &PgPool,
    entity_id: Uuid,
) -> anyhow::Result<Vec<VisitHeritageAuditRow>> {
    let rows = sqlx::query_as(
        r#"
        SELECT
            ce.id,
            ce.event_type,
            ce.title,
            ce.occurrence_key,
            ST_Y(ce.geom::geometry) AS lat,
            ST_X(ce.geom::geometry) AS lon,
            COALESCE(ce.evidence_count, 0) AS evidence_count
        FROM canonical_events ce
        WHERE ce.entity_id = $1
          AND ce.pipeline = 'person'
          AND ce.is_active
          AND ce.visit_eligible
        ORDER BY ce.event_type, ce.title
        "#,
    )
    .bind(entity_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn visit_heritage_source_counts(
    pool: &PgPool,
    entity_id: Uuid,
) -> anyhow::Result<(i64, i64, i64, i64, i64, i64)> {
    let row: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE visit_eligible)::bigint,
            COUNT(*) FILTER (WHERE visit_eligible AND occurrence_key LIKE 'visit-fixture:%')::bigint,
            COUNT(*) FILTER (WHERE visit_eligible AND occurrence_key LIKE 'visit-wikidata:%')::bigint,
            COUNT(*) FILTER (WHERE visit_eligible AND COALESCE(occurrence_key, '') NOT LIKE 'visit-%')::bigint,
            COUNT(*) FILTER (WHERE timeline_eligible)::bigint,
            COUNT(*) FILTER (WHERE map_eligible)::bigint
        FROM canonical_events
        WHERE entity_id = $1 AND pipeline = 'person' AND is_active
        "#,
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn deactivate_canonical_event(pool: &PgPool, event_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE canonical_events SET is_active = false
        WHERE id = $1 AND pipeline = 'person'
        "#,
    )
    .bind(event_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn count_visit_heritage(pool: &PgPool, entity_id: Uuid) -> anyhow::Result<i64> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint FROM canonical_events
        WHERE entity_id = $1 AND is_active AND pipeline = 'person' AND visit_eligible
        "#,
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::eligibility_for_event_type;

    #[test]
    fn commemorative_types_are_visit_only() {
        assert_eq!(eligibility_for_event_type("museum"), (false, true));
        assert_eq!(eligibility_for_event_type("battle"), (true, false));
    }
}
