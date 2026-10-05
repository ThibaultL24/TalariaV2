//! Entity-scoped projections. Source/canonical data is never modified here.
use super::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Postgres, QueryBuilder};
use talaria_store::CanonicalEventRow;
use uuid::Uuid;

#[derive(Debug, Deserialize, Default)]
pub struct ViewQuery {
    from: Option<i32>,
    to: Option<i32>,
    types: Option<String>,
    bbox: Option<String>,
    cursor: Option<String>,
    limit: Option<i64>,
    lang: Option<String>,
    resolution: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Cursor {
    entity: Uuid,
    from: Option<i32>,
    to: Option<i32>,
    types: Vec<String>,
    bbox: Option<[f64; 4]>,
    map: bool,
    resolution: String,
    time: Option<DateTime<Utc>>,
    id: Uuid,
}

fn error(status: StatusCode, code: &str) -> Response {
    (status, Json(json!({"error": code}))).into_response()
}

fn parse_bbox(value: Option<&str>) -> Result<Option<[f64; 4]>, &'static str> {
    let Some(value) = value else { return Ok(None) };
    let numbers: Vec<f64> = value
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| "invalid_bbox")?;
    if numbers.len() != 4 || numbers.iter().any(|n| !n.is_finite()) {
        return Err("invalid_bbox");
    }
    let [west, south, east, north] = [numbers[0], numbers[1], numbers[2], numbers[3]];
    if !(-180.0..=180.0).contains(&west)
        || !(-180.0..=180.0).contains(&east)
        || !(-90.0..=90.0).contains(&south)
        || !(-90.0..=90.0).contains(&north)
        || west > east
        || south > north
    {
        return Err("invalid_bbox");
    }
    Ok(Some([west, south, east, north]))
}

pub async fn timeline(
    State(state): State<AppState>,
    Path(entity): Path<Uuid>,
    Query(query): Query<ViewQuery>,
) -> Response {
    view(state, entity, query, false).await
}
pub async fn map(
    State(state): State<AppState>,
    Path(entity): Path<Uuid>,
    Query(query): Query<ViewQuery>,
) -> Response {
    view(state, entity, query, true).await
}

async fn view(state: AppState, entity: Uuid, query: ViewQuery, map: bool) -> Response {
    let resolution = query.resolution.as_deref().unwrap_or("detail");
    if !matches!(resolution, "overview" | "period" | "detail") {
        return error(StatusCode::BAD_REQUEST, "invalid_resolution");
    }
    let limit = query.limit.unwrap_or(200);
    if !(1..=500).contains(&limit) {
        return error(StatusCode::BAD_REQUEST, "invalid_limit");
    }
    if query.from.zip(query.to).is_some_and(|(a, b)| a > b)
        || query.from.is_some_and(|n| !(-9999..=9999).contains(&n))
        || query.to.is_some_and(|n| !(-9999..=9999).contains(&n))
    {
        return error(StatusCode::BAD_REQUEST, "invalid_period");
    }
    let bbox = match parse_bbox(query.bbox.as_deref()) {
        Ok(v) => v,
        Err(code) => return error(StatusCode::BAD_REQUEST, code),
    };
    if !map && bbox.is_some() {
        return error(StatusCode::BAD_REQUEST, "bbox_requires_map");
    }
    let mut types: Vec<String> = query
        .types
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if types.len() > 32
        || types
            .iter()
            .any(|s| s.len() > 64 || !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
    {
        return error(StatusCode::BAD_REQUEST, "invalid_types");
    }
    types.sort();
    types.dedup();
    let cursor = match query.cursor.as_deref() {
        None => None,
        Some(value) => {
            if value.len() > 8192 {
                return error(StatusCode::BAD_REQUEST, "invalid_cursor");
            }
            let decoded = hex::decode(value)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Cursor>(&bytes).ok());
            match decoded {
                Some(c)
                    if c.entity == entity
                        && c.from == query.from
                        && c.to == query.to
                        && c.types == types
                        && c.bbox == bbox
                        && c.map == map
                        && c.resolution == resolution =>
                {
                    Some(c)
                }
                _ => return error(StatusCode::BAD_REQUEST, "invalid_cursor"),
            }
        }
    };
    match talaria_store::get_entity(&state.pool, entity).await {
        Ok(Some(_)) => (),
        Ok(None) => return error(StatusCode::NOT_FOUND, "entity_not_found"),
        Err(e) => {
            tracing::error!(error=%e, "entity view failed");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "database_error");
        }
    }
    let mut sql = QueryBuilder::<Postgres>::new("SELECT ce.id, ce.entity_id, COALESCE(e.canonical_name,e.wikipedia_title) AS person_name, ce.event_type, ce.epistemic_status, ce.title, ce.summary, ce.start_time, ce.time_json, ce.place_label, ce.confidence, ce.map_eligible, ST_Y(ce.geom::geometry) AS lat, ST_X(ce.geom::geometry) AS lon FROM person_timeline_projection ce JOIN entities e ON e.id=ce.entity_id WHERE ce.entity_id=");
    sql.push_bind(entity);
    // Life of the person. Memorials and later honors stay out of this view.
    sql.push(
        " AND ce.event_type NOT IN ('statue','museum','memorial','street_naming','commemoration')",
    );
    if !map {
        let threshold = match resolution {
            "overview" => 0.8f64,
            "period" => 0.6,
            _ => 0.0,
        };
        sql.push(" AND ce.timeline_importance >= ")
            .push_bind(threshold);
    }
    if !types.is_empty() {
        sql.push(" AND ce.event_type=ANY(")
            .push_bind(types.clone())
            .push(")");
    }
    // Year projections preserve typed source dates in the returned time_json.
    if let Some(from) = query.from {
        sql.push(" AND COALESCE(substring(ce.time_json->>'end' from '^(-?[0-9]{4})')::int, EXTRACT(YEAR FROM ce.start_time)::int) >= ").push_bind(from);
    }
    if let Some(to) = query.to {
        sql.push(" AND EXTRACT(YEAR FROM ce.start_time)::int <= ")
            .push_bind(to);
    }
    if map {
        sql.push(" AND ce.map_eligible AND ce.geom IS NOT NULL");
        if let Some([west, south, east, north]) = bbox {
            sql.push(" AND ST_Intersects(ce.geom, ST_MakeEnvelope(")
                .push_bind(west)
                .push(",")
                .push_bind(south)
                .push(",")
                .push_bind(east)
                .push(",")
                .push_bind(north)
                .push(",4326)::geography)");
        }
    }
    if let Some(c) = cursor {
        match c.time {
            Some(time) => {
                sql.push(" AND (ce.start_time IS NULL OR (ce.start_time,ce.id) > (")
                    .push_bind(time)
                    .push(",")
                    .push_bind(c.id)
                    .push("))");
            }
            None => {
                sql.push(" AND ce.start_time IS NULL AND ce.id > ")
                    .push_bind(c.id);
            }
        }
    }
    sql.push(" ORDER BY ce.start_time ASC NULLS LAST, ce.id ASC LIMIT ")
        .push_bind(limit + 1);
    let mut events = match sql
        .build_query_as::<CanonicalEventRow>()
        .fetch_all(&state.pool)
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!(error=%e, "entity events failed");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "database_error");
        }
    };
    let has_more = events.len() > limit as usize;
    events.truncate(limit as usize);
    let next = if has_more {
        events.last().map(|last| {
            hex::encode(
                serde_json::to_vec(&Cursor {
                    entity,
                    from: query.from,
                    to: query.to,
                    types,
                    bbox,
                    map,
                    resolution: resolution.into(),
                    time: last.start_time,
                    id: last.id,
                })
                .expect("cursor serialization"),
            )
        })
    } else {
        None
    };
    let mut items: Vec<Value> = events
        .iter()
        .map(super::events::event_to_json_list)
        .collect();
    if let Some(lang) = crate::display_i18n::normalize_ui_lang(query.lang.as_deref()) {
        crate::display_i18n::localize_event_list_items(&mut items, lang);
    }
    if map {
        let features: Vec<Value> = events.iter().zip(items).filter_map(|(event, properties)| Some(json!({"type":"Feature","id":event.id,"geometry":{"type":"Point","coordinates":[event.lon?,event.lat?]},"properties":properties}))).collect();
        Json(json!({"type":"FeatureCollection","features":features,"pagination":{"next_cursor":next}})).into_response()
    } else {
        Json(json!({"events":items,"count":events.len(),"pagination":{"next_cursor":next}}))
            .into_response()
    }
}

pub async fn overview(State(state): State<AppState>, Path(entity_id): Path<Uuid>) -> Response {
    let entity = match talaria_store::get_entity(&state.pool, entity_id).await {
        Ok(Some(e)) => e,
        Ok(None) => return error(StatusCode::NOT_FOUND, "entity_not_found"),
        Err(_) => return error(StatusCode::INTERNAL_SERVER_ERROR, "database_error"),
    };
    let stats = sqlx::query_as::<_, (i64,i64,i64,Option<i32>,Option<i32>)>("SELECT count(*)::bigint, count(*) FILTER(WHERE map_eligible)::bigint, count(DISTINCT place_label)::bigint, min(EXTRACT(YEAR FROM start_time)::int), max(COALESCE(substring(time_json->>'end' from '^(-?[0-9]{4})')::int,EXTRACT(YEAR FROM start_time)::int)) FROM canonical_events WHERE entity_id=$1 AND is_active AND timeline_eligible AND pipeline='person'")
        .bind(entity_id).fetch_one(&state.pool).await;
    match stats {
        Ok((timeline,mapped,places,from,to)) => Json(json!({"entity":{"id":entity.id,"qid":entity.qid,"label":entity.canonical_name.unwrap_or(entity.wikipedia_title)},"stats":{"timeline_events":timeline,"mapped_events":mapped,"places":places},"time_bounds":{"from":from,"to":to}})).into_response(),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "database_error"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bbox_rejects_non_finite_reversed_and_out_of_bounds() {
        for value in ["NaN,0,1,1", "10,0,-10,1", "0,91,1,92", "0,1,2"] {
            assert!(parse_bbox(Some(value)).is_err());
        }
        assert_eq!(
            parse_bbox(Some("-5,42,9,51")).unwrap(),
            Some([-5., 42., 9., 51.])
        );
    }
    #[test]
    fn cursor_preserves_unknown_time() {
        let c = Cursor {
            entity: Uuid::nil(),
            from: None,
            to: None,
            types: vec![],
            bbox: None,
            map: false,
            resolution: "detail".into(),
            time: None,
            id: Uuid::nil(),
        };
        let bytes = serde_json::to_vec(&c).unwrap();
        assert!(serde_json::from_slice::<Cursor>(&bytes)
            .unwrap()
            .time
            .is_none());
    }
    fn state(pool: sqlx::PgPool) -> AppState {
        AppState {
            pool,
            offline_only: true,
            config: talaria_core::AppConfig::from_env().unwrap(),
            ingest_jobs: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
        }
    }
    async fn payload(response: Response) -> Value {
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 1_000_000)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }
    async fn seed(pool: &sqlx::PgPool) -> Uuid {
        let entity: Uuid = sqlx::query_scalar("INSERT INTO entities(wikipedia_title,canonical_name) VALUES('V3 fixture','V3 fixture') RETURNING id").fetch_one(pool).await.unwrap();
        for (ordinal, event_type, date, time, eligible) in [
            (
                1,
                "birth",
                Some("1802-01-01"),
                json!({"kind":"exact","start":"1802","precision":"year"}),
                true,
            ),
            (
                2,
                "travel",
                Some("1855-01-01"),
                json!({"kind":"range","start":"1855","end":"1870","precision":"year"}),
                true,
            ),
            (
                3,
                "publication",
                Some("1855-01-01"),
                json!({"kind":"approx","start":"1855","precision":"year"}),
                true,
            ),
            (4, "other", None, json!({"kind":"unknown"}), true),
            (5, "other", None, json!({"kind":"unknown"}), false),
        ] {
            sqlx::query("INSERT INTO canonical_events(id,entity_id,event_type,title,start_time,time_json,pipeline,timeline_eligible,map_eligible,geom) VALUES($1,$2,$3,'Fixture',$4::text::timestamptz,$5,'person',$6,true,ST_SetSRID(ST_MakePoint(2.35,48.85),4326)::geography)")
                .bind(Uuid::from_u128(ordinal)).bind(entity).bind(event_type).bind(date).bind(time).bind(eligible).execute(pool).await.unwrap();
        }
        entity
    }
    #[sqlx::test(migrations = "../../migrations")]
    async fn v3_db_cursor_no_duplicates_or_missing_unknown_dates(pool: sqlx::PgPool) {
        let entity = seed(&pool).await;
        let state = state(pool);
        let mut query = ViewQuery {
            limit: Some(1),
            ..Default::default()
        };
        let mut ids = Vec::new();
        loop {
            let result = payload(view(state.clone(), entity, query, false).await).await;
            for item in result["events"].as_array().unwrap() {
                ids.push(item["id"].as_str().unwrap().to_string());
            }
            let next = result["pagination"]["next_cursor"]
                .as_str()
                .map(str::to_string);
            if next.is_none() {
                break;
            }
            query = ViewQuery {
                limit: Some(1),
                cursor: next,
                ..Default::default()
            };
        }
        assert_eq!(ids.len(), 4);
        assert_eq!(
            ids.iter().collect::<std::collections::HashSet<_>>().len(),
            4
        );
        assert_eq!(ids.last().unwrap(), &Uuid::from_u128(4).to_string());
    }
    #[sqlx::test(migrations = "../../migrations")]
    async fn v3_db_bbox_and_range_overlap(pool: sqlx::PgPool) {
        let entity = seed(&pool).await;
        let state = state(pool);
        let result = payload(
            view(
                state.clone(),
                entity,
                ViewQuery {
                    from: Some(1860),
                    to: Some(1865),
                    ..Default::default()
                },
                false,
            )
            .await,
        )
        .await;
        assert_eq!(result["events"].as_array().unwrap().len(), 1);
        assert_eq!(result["events"][0]["time"]["kind"], "range");
        let result = payload(
            view(
                state,
                entity,
                ViewQuery {
                    bbox: Some("10,0,20,10".into()),
                    ..Default::default()
                },
                true,
            )
            .await,
        )
        .await;
        assert!(result["features"].as_array().unwrap().is_empty());
    }
    #[sqlx::test(migrations = "../../migrations")]
    async fn v3_db_semantic_zoom_is_nested(pool: sqlx::PgPool) {
        let entity = seed(&pool).await;
        let state = state(pool);
        let mut levels = Vec::new();
        for resolution in ["overview", "period", "detail"] {
            let result = payload(
                view(
                    state.clone(),
                    entity,
                    ViewQuery {
                        resolution: Some(resolution.into()),
                        ..Default::default()
                    },
                    false,
                )
                .await,
            )
            .await;
            levels.push(
                result["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| e["id"].as_str().unwrap().to_string())
                    .collect::<std::collections::HashSet<_>>(),
            );
        }
        assert!(levels[0].is_subset(&levels[1]));
        assert!(levels[1].is_subset(&levels[2]));
        assert_eq!(levels[2].len(), 4);
    }
}
