// crates/talaria-api/src/routes/visit.rs
use super::events::event_to_json_list;
use super::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use talaria_store::{
    list_visit_heritage, list_visit_now, CanonicalEventRow, VisitOpportunityRow,
};
use uuid::Uuid;

#[derive(Debug, Deserialize, Default)]
pub struct VisitQuery {
    bbox: Option<String>,
    limit: Option<i64>,
    lang: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct VisitNowQuery {
    from: Option<String>,
    to: Option<String>,
    radius_km: Option<f64>,
    bbox: Option<String>,
    limit: Option<i64>,
}

fn error(status: StatusCode, code: &str) -> Response {
    (status, Json(json!({"error": code}))).into_response()
}

fn parse_bbox(value: Option<&str>) -> Result<Option<[f64; 4]>, &'static str> {
    let Some(value) = value else {
        return Ok(None);
    };
    let numbers: Vec<f64> = value
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| "invalid_bbox")?;
    if numbers.len() != 4 || numbers.iter().any(|n| !n.is_finite()) {
        return Err("invalid_bbox");
    }
    let [west, south, east, north] = [numbers[0], numbers[1], numbers[2], numbers[3]];
    if west > east || south > north {
        return Err("invalid_bbox");
    }
    Ok(Some([west, south, east, north]))
}

async fn ensure_entity(state: &AppState, entity_id: Uuid) -> Result<(), Response> {
    match talaria_store::get_entity(&state.pool, entity_id).await {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(error(StatusCode::NOT_FOUND, "entity_not_found")),
        Err(_) => Err(error(StatusCode::INTERNAL_SERVER_ERROR, "database_error")),
    }
}

pub async fn heritage(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
    Query(query): Query<VisitQuery>,
) -> Response {
    let bbox = match parse_bbox(query.bbox.as_deref()) {
        Ok(b) => b,
        Err(code) => return error(StatusCode::BAD_REQUEST, code),
    };
    if let Err(response) = ensure_entity(&state, entity_id).await {
        return response;
    }
    let limit = query.limit.unwrap_or(200).clamp(1, 500);
    let events = match list_visit_heritage(&state.pool, entity_id, limit, bbox, false).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!(error=%e, "visit heritage failed");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "database_error");
        }
    };
    let mut items: Vec<Value> = events.iter().map(event_to_json_list).collect();
    if let Some(lang) = crate::display_i18n::normalize_ui_lang(query.lang.as_deref()) {
        crate::display_i18n::localize_event_list_items(&mut items, lang);
    }
    Json(json!({
        "events": items,
        "count": items.len(),
    }))
        .into_response()
}

pub async fn heritage_geojson(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
    Query(query): Query<VisitQuery>,
) -> Response {
    let bbox = match parse_bbox(query.bbox.as_deref()) {
        Ok(b) => b,
        Err(code) => return error(StatusCode::BAD_REQUEST, code),
    };
    if let Err(response) = ensure_entity(&state, entity_id).await {
        return response;
    }
    let limit = query.limit.unwrap_or(500).clamp(1, 500);
    let events = match list_visit_heritage(&state.pool, entity_id, limit, bbox, true).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!(error=%e, "visit geojson failed");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "database_error");
        }
    };
    let features: Vec<Value> = events
        .iter()
        .map(geojson_feature)
        .filter_map(Result::ok)
        .collect();
    Json(json!({
        "type": "FeatureCollection",
        "features": features,
        "count": features.len(),
    }))
        .into_response()
}

fn geojson_feature(event: &CanonicalEventRow) -> Result<Value, ()> {
    let (lat, lon) = (event.lat.ok_or(())?, event.lon.ok_or(())?);
    Ok(json!({
        "type": "Feature",
        "id": event.id,
        "geometry": {"type": "Point", "coordinates": [lon, lat]},
        "properties": event_to_json_list(event),
    }))
}

fn opportunity_to_json(row: &VisitOpportunityRow) -> Value {
    let mut value = json!({
        "id": row.id,
        "kind": row.kind,
        "title": row.title,
        "summary": row.summary,
        "venue_label": row.venue_label,
        "starts_at": row.starts_at,
        "ends_at": row.ends_at,
        "canonical_url": row.canonical_url,
        "source_kind": row.source_kind,
        "source_record_id": row.source_record_id,
        "fetched_at": row.fetched_at,
    });
    if let (Some(lat), Some(lon)) = (row.lat, row.lon) {
        value["coordinates"] = json!({ "lat": lat, "lon": lon });
    }
    value
}

fn parse_window(
    from: Option<&str>,
    to: Option<&str>,
) -> Result<(chrono::DateTime<Utc>, chrono::DateTime<Utc>), &'static str> {
    let now = Utc::now();
    let window_start = if let Some(s) = from {
        chrono::DateTime::parse_from_rfc3339(s)
            .map_err(|_| "invalid_from")?
            .with_timezone(&Utc)
    } else {
        now - Duration::days(7)
    };
    let window_end = if let Some(s) = to {
        chrono::DateTime::parse_from_rfc3339(s)
            .map_err(|_| "invalid_to")?
            .with_timezone(&Utc)
    } else {
        now + Duration::days(90)
    };
    if window_start > window_end {
        return Err("invalid_window");
    }
    Ok((window_start, window_end))
}

pub async fn now(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
    Query(query): Query<VisitNowQuery>,
) -> Response {
    let (window_start, window_end) = match parse_window(query.from.as_deref(), query.to.as_deref()) {
        Ok(w) => w,
        Err(code) => return error(StatusCode::BAD_REQUEST, code),
    };
    let radius_km = query.radius_km.unwrap_or(50.0).clamp(1.0, 500.0);
    let radius_meters = radius_km * 1000.0;
    let limit = query.limit.unwrap_or(200).clamp(1, 500);
    if let Err(response) = ensure_entity(&state, entity_id).await {
        return response;
    }
    let rows = match list_visit_now(
        &state.pool,
        entity_id,
        window_start,
        window_end,
        radius_meters,
        limit,
        None,
        false,
    )
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!(error=%e, "visit now failed");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "database_error");
        }
    };
    let items: Vec<Value> = rows.iter().map(opportunity_to_json).collect();
    Json(json!({
        "opportunities": items,
        "count": items.len(),
        "window": {
            "from": window_start,
            "to": window_end,
        },
        "radius_km": radius_km,
    }))
        .into_response()
}

pub async fn now_geojson(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
    Query(query): Query<VisitNowQuery>,
) -> Response {
    let bbox = match parse_bbox(query.bbox.as_deref()) {
        Ok(b) => b,
        Err(code) => return error(StatusCode::BAD_REQUEST, code),
    };
    let (window_start, window_end) = match parse_window(query.from.as_deref(), query.to.as_deref()) {
        Ok(w) => w,
        Err(code) => return error(StatusCode::BAD_REQUEST, code),
    };
    let radius_km = query.radius_km.unwrap_or(50.0).clamp(1.0, 500.0);
    let radius_meters = radius_km * 1000.0;
    let limit = query.limit.unwrap_or(500).clamp(1, 500);
    if let Err(response) = ensure_entity(&state, entity_id).await {
        return response;
    }
    let rows = match list_visit_now(
        &state.pool,
        entity_id,
        window_start,
        window_end,
        radius_meters,
        limit,
        bbox,
        true,
    )
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!(error=%e, "visit now geojson failed");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "database_error");
        }
    };
    let features: Vec<Value> = rows
        .iter()
        .filter_map(|row| {
            let (lat, lon) = match (row.lat, row.lon) {
                (Some(lat), Some(lon)) => (lat, lon),
                _ => return None,
            };
            let props = opportunity_to_json(row);
            Some(json!({
                "type": "Feature",
                "id": row.id,
                "geometry": {"type": "Point", "coordinates": [lon, lat]},
                "properties": {
                    "id": row.id,
                    "title": row.title,
                    "event_type": "visit_opportunity",
                    "visit_layer": "now",
                    "kind": row.kind,
                    "venue_label": row.venue_label,
                    "starts_at": row.starts_at,
                    "ends_at": row.ends_at,
                    "canonical_url": row.canonical_url,
                    "source_kind": row.source_kind,
                    "detail": props,
                },
            }))
        })
        .collect();
    Json(json!({
        "type": "FeatureCollection",
        "features": features,
        "count": features.len(),
    }))
        .into_response()
}

pub async fn anchors(State(state): State<AppState>, Path(entity_id): Path<Uuid>) -> Response {
    if let Err(response) = ensure_entity(&state, entity_id).await {
        return response;
    }
    let rows = sqlx::query_as::<_, (f64, f64)>(
        r#"
        SELECT ST_Y(geom::geometry) AS lat, ST_X(geom::geometry) AS lon
        FROM canonical_events
        WHERE entity_id = $1 AND is_active AND pipeline = 'person'
          AND map_eligible AND geom IS NOT NULL
          AND (timeline_eligible OR visit_eligible)
        LIMIT 500
        "#,
    )
    .bind(entity_id)
    .fetch_all(&state.pool)
    .await;
    match rows {
        Ok(points) => {
            let features: Vec<Value> = points
                .into_iter()
                .enumerate()
                .map(|(i, (lat, lon))| {
                    json!({
                        "type": "Feature",
                        "id": format!("anchor-{i}"),
                        "geometry": {"type": "Point", "coordinates": [lon, lat]},
                    })
                })
                .collect();
            Json(json!({
                "type": "FeatureCollection",
                "features": features,
                "count": features.len(),
            }))
                .into_response()
        }
        Err(e) => {
            tracing::error!(error=%e, "visit anchors failed");
            error(StatusCode::INTERNAL_SERVER_ERROR, "database_error")
        }
    }
}
