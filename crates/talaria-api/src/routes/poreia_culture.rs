// crates/talaria-api/src/routes/poreia_culture.rs
use super::poreia_query::{parse_bbox, parse_geo, parse_range};
use super::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use talaria_store::{list_published_occurrences, PoreiaOccurrenceRow};

#[derive(Debug, Deserialize)]
pub struct OccurrencesQuery {
    lat: Option<f64>,
    lon: Option<f64>,
    radius_km: Option<f64>,
    from: Option<String>,
    to: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    bbox: Option<String>,
}

fn error(status: StatusCode, code: &'static str) -> Response {
    (status, Json(json!({ "error": code }))).into_response()
}

fn day(value: chrono::NaiveDate) -> String {
    value.format("%Y-%m-%d").to_string()
}

fn occurrence_json(row: &PoreiaOccurrenceRow) -> Value {
    let mut item = json!({
        "id": row.id,
        "kind": row.kind,
        "title": row.title,
        "summary": row.summary,
        "venue_label": row.venue_label,
        "time_precision": row.time_precision,
        "source_url": row.source_url,
    });
    if let Some(date) = row.occurs_on {
        item["occurs_on"] = json!(day(date));
    }
    if let Some(date) = row.ends_on {
        item["ends_on"] = json!(day(date));
    }
    if let (Some(lat), Some(lon)) = (row.lat, row.lon) {
        item["coordinates"] = json!({ "lat": lat, "lon": lon });
    }
    item
}

pub async fn occurrences(
    State(state): State<AppState>,
    Query(query): Query<OccurrencesQuery>,
) -> Response {
    let geo = match parse_geo(
        query.lat,
        query.lon,
        query.radius_km,
        query.limit,
        query.offset,
    ) {
        Ok(geo) => geo,
        Err(code) => return error(StatusCode::BAD_REQUEST, code),
    };
    let (from, to) = match (query.from.as_deref(), query.to.as_deref()) {
        (Some(from), Some(to)) => match parse_range(from, to) {
            Ok(range) => range,
            Err(code) => return error(StatusCode::BAD_REQUEST, code),
        },
        _ => return error(StatusCode::BAD_REQUEST, "invalid_date"),
    };
    let bbox = match query.bbox.as_deref() {
        Some(value) => match parse_bbox(value) {
            Ok(bbox) => Some(bbox),
            Err(code) => return error(StatusCode::BAD_REQUEST, code),
        },
        None => None,
    };
    let fetched = match list_published_occurrences(
        &state.pool,
        geo.lon,
        geo.lat,
        geo.radius_m,
        from,
        to,
        geo.limit + 1,
        geo.offset,
        bbox,
    )
    .await
    {
        Ok(rows) => rows,
        Err(db_error) => {
            tracing::error!(%db_error, "poreia occurrence catalog failed");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "database_error");
        }
    };
    let has_more = fetched.len() > geo.limit as usize;
    let items: Vec<Value> = fetched
        .iter()
        .take(geo.limit as usize)
        .map(occurrence_json)
        .collect();
    Json(json!({
        "items": items,
        "limit": geo.limit,
        "offset": geo.offset,
        "has_more": has_more,
        "from": from.format("%Y-%m-%d").to_string(),
        "to": to.format("%Y-%m-%d").to_string(),
    }))
    .into_response()
}

#[cfg(test)]
mod tests {
    use crate::routes::poreia_query::{parse_day, parse_range};

    #[test]
    fn day_precision_rejects_a_clock_time() {
        assert!(parse_day("2024-06-01").is_ok());
        assert_eq!(
            parse_day("2024-06-01T00:00:00Z").unwrap_err(),
            "invalid_date"
        );
        assert_eq!(parse_day("2024-06-01 10:00").unwrap_err(), "invalid_date");
        assert!(parse_range("2024-09-08", "2024-06-01").is_err());
        assert!(parse_range("2024-06-01", "2026-06-01").is_err());
    }
}
