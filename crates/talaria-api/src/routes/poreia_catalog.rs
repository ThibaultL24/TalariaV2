// crates/talaria-api/src/routes/poreia_catalog.rs
use super::poreia_query::{parse_bbox, parse_geo, parse_lang};
use super::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use talaria_store::{list_published_sites, PoreiaSiteRow};

#[derive(Debug, Deserialize)]
pub struct SitesQuery {
    lat: Option<f64>,
    lon: Option<f64>,
    radius_km: Option<f64>,
    lang: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    bbox: Option<String>,
}

fn error(status: StatusCode, code: &'static str) -> Response {
    (status, Json(json!({ "error": code }))).into_response()
}

fn site_json(row: &PoreiaSiteRow) -> Value {
    let mut item = json!({
        "id": row.id,
        "wikidata_qid": row.wikidata_qid,
        "label": row.label,
        "kind": row.kind,
        "visitability": row.visitability,
        "coordinates": { "lat": row.lat, "lon": row.lon },
        "historical_links": row.narratives,
        "media": row.media,
        "source_url": format!("https://www.wikidata.org/wiki/{}", row.wikidata_qid),
    });
    if let (Some(hours), Some(checked_at)) = (&row.opening_hours, row.hours_checked_at) {
        let trimmed = hours.trim();
        if !trimmed.is_empty() {
            item["opening_hours"] = json!(trimmed);
            item["hours_checked_at"] = json!(checked_at.to_rfc3339());
        }
    }
    item
}

pub async fn sites(State(state): State<AppState>, Query(query): Query<SitesQuery>) -> Response {
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
    let lang = match parse_lang(query.lang.as_deref()) {
        Ok(lang) => lang,
        Err(code) => return error(StatusCode::BAD_REQUEST, code),
    };
    let bbox = match query.bbox.as_deref() {
        Some(value) => match parse_bbox(value) {
            Ok(bbox) => Some(bbox),
            Err(code) => return error(StatusCode::BAD_REQUEST, code),
        },
        None => None,
    };
    let fetched = match list_published_sites(
        &state.pool,
        geo.lon,
        geo.lat,
        geo.radius_m,
        geo.limit + 1,
        geo.offset,
        bbox,
    )
    .await
    {
        Ok(rows) => rows,
        Err(db_error) => {
            tracing::error!(%db_error, "poreia site catalog failed");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "database_error");
        }
    };
    let has_more = fetched.len() > geo.limit as usize;
    let items: Vec<Value> = fetched
        .iter()
        .take(geo.limit as usize)
        .map(site_json)
        .collect();
    Json(json!({
        "items": items,
        "limit": geo.limit,
        "offset": geo.offset,
        "has_more": has_more,
        "lang": lang,
    }))
    .into_response()
}

#[cfg(test)]
mod tests {
    use crate::routes::poreia_query::parse_geo;

    #[test]
    fn geographic_bounds_reject_missing_and_extreme_values() {
        assert_eq!(
            parse_geo(None, Some(2.3), Some(10.0), None, None).unwrap_err(),
            "invalid_geo_query"
        );
        assert_eq!(
            parse_geo(Some(91.0), Some(2.3), Some(10.0), None, None).unwrap_err(),
            "invalid_latitude"
        );
        assert_eq!(
            parse_geo(Some(48.8), Some(2.3), Some(80.0), None, None).unwrap_err(),
            "invalid_radius"
        );
        let window = parse_geo(Some(48.8566), Some(2.3522), Some(10.0), Some(50), Some(0)).unwrap();
        assert_eq!(window.radius_m, 10_000.0);
        assert_eq!(window.limit, 50);
    }
}
