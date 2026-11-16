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
    group: Option<String>,
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
    #[serde(default)]
    group: Option<String>,
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
    let group = query
        .group
        .as_deref()
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(str::to_string);
    if group
        .as_deref()
        .is_some_and(|slug| !talaria_store::is_editorial_slug(slug))
    {
        return error(StatusCode::BAD_REQUEST, "invalid_group");
    }
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
                        && c.resolution == resolution
                        && c.group.as_deref() == group.as_deref() =>
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
    let group_id = if let Some(slug) = group.as_deref() {
        match talaria_store::find_published_editorial_group(&state.pool, entity, slug).await {
            Ok(Some(found)) => Some(found.id),
            Ok(None) => return error(StatusCode::NOT_FOUND, "group_not_found"),
            Err(e) => {
                tracing::error!(error=%e, "editorial group lookup failed");
                return error(StatusCode::INTERNAL_SERVER_ERROR, "database_error");
            }
        }
    } else {
        None
    };
    let mut sql = QueryBuilder::<Postgres>::new("SELECT ce.id, ce.entity_id, COALESCE(e.canonical_name,e.wikipedia_title) AS person_name, ce.event_type, ce.epistemic_status, ce.title, ce.summary, ce.start_time, ce.time_json, ce.place_label, ce.confidence, ce.map_eligible, ST_Y(ce.geom::geometry) AS lat, ST_X(ce.geom::geometry) AS lon, ce.coord_precision_level, ce.uncertainty_radius_m ");
    if let Some(group_id) = group_id {
        // A chapter lists the active version of each chosen fact.
        // The biography overview threshold does not apply inside it.
        sql.push("FROM canonical_events ce JOIN entities e ON e.id=ce.entity_id JOIN editorial_memberships m ON m.logical_event_id=ce.logical_event_id AND m.group_id=")
            .push_bind(group_id);
        sql.push(" WHERE ce.entity_id=")
            .push_bind(entity)
            .push(" AND ce.is_active AND ce.pipeline='person'");
    } else {
        sql.push("FROM person_timeline_projection ce JOIN entities e ON e.id=ce.entity_id WHERE ce.entity_id=")
            .push_bind(entity);
        // Life of the person. Memorials and later honors stay out of the general view.
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
                    group: group.clone(),
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
            group: None,
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

    struct PlacedFact {
        canonical: Uuid,
        logical: Uuid,
    }

    async fn person(pool: &sqlx::PgPool, name: &str) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO entities(wikipedia_title, canonical_name) VALUES($1, $1) RETURNING id",
        )
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn fact(
        pool: &sqlx::PgPool,
        entity: Uuid,
        event_type: &str,
        title: &str,
        time: Value,
        start: Option<&str>,
        place: Option<&str>,
        point: Option<(f64, f64)>,
        status: &str,
        precision: Option<&str>,
        radius: Option<f64>,
    ) -> PlacedFact {
        let logical: Uuid =
            sqlx::query_scalar("INSERT INTO logical_events DEFAULT VALUES RETURNING id")
                .fetch_one(pool)
                .await
                .unwrap();
        let (lon, lat) = point.unzip();
        let canonical: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO canonical_events (
                entity_id, event_type, title, start_time, time_json, pipeline, is_active,
                timeline_eligible, map_eligible, epistemic_status, logical_event_id,
                place_label, coord_precision_level, uncertainty_radius_m, geom
            )
            VALUES (
                $1, $2, $3, $4::text::timestamptz, $5, 'person', true,
                true, $6, $7, $8,
                $9, $10, $11,
                CASE WHEN $12::float8 IS NULL THEN NULL
                     ELSE ST_SetSRID(ST_MakePoint($13, $12), 4326)::geography END
            )
            RETURNING id
            "#,
        )
        .bind(entity)
        .bind(event_type)
        .bind(title)
        .bind(start)
        .bind(time)
        .bind(point.is_some())
        .bind(status)
        .bind(logical)
        .bind(place)
        .bind(precision)
        .bind(radius)
        .bind(lat)
        .bind(lon)
        .fetch_one(pool)
        .await
        .unwrap();
        PlacedFact { canonical, logical }
    }

    async fn chapter(
        pool: &sqlx::PgPool,
        entity: Uuid,
        slug: &str,
        title: &str,
        kind: &str,
        summary: Option<&str>,
        origin: Option<&str>,
        order: i32,
        publish: bool,
    ) -> Uuid {
        let id = talaria_store::create_editorial_group(
            pool,
            talaria_store::NewEditorialGroup {
                entity_id: entity,
                slug,
                title,
                group_kind: kind,
                summary,
                summary_origin: origin,
                display_order: order,
            },
        )
        .await
        .unwrap();
        if publish {
            talaria_store::set_editorial_group_status(pool, id, talaria_store::STATUS_PUBLISHED)
                .await
                .unwrap();
        }
        id
    }

    async fn json_status(response: Response) -> (StatusCode, Value) {
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 2_000_000)
            .await
            .unwrap();
        let body = if bytes.is_empty() {
            json!({})
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, body)
    }

    fn event_ids(body: &Value) -> Vec<String> {
        body["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap().to_string())
            .collect()
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn published_chapters_filter_timeline_and_map(pool: sqlx::PgPool) {
        let napoleon = person(&pool, "Napoleon fixture").await;
        let hugo = person(&pool, "Victor Hugo fixture").await;
        let curie = person(&pool, "Marie Curie fixture").await;
        let aristotle = person(&pool, "Aristotle fixture").await;
        let birth = fact(
            &pool,
            napoleon,
            "birth",
            "Birth",
            json!({"kind":"exact","precision":"year","start":"1769"}),
            Some("1769-01-01"),
            Some("Ajaccio"),
            Some((8.73, 41.92)),
            "attested",
            Some("point"),
            None,
        )
        .await;
        let battle = fact(
            &pool,
            napoleon,
            "battle",
            "Battle of Lodi",
            json!({"kind":"exact","precision":"day","start":"1796-05-10"}),
            Some("1796-05-10"),
            Some("Lodi"),
            Some((9.5, 45.3)),
            "attested",
            Some("point"),
            None,
        )
        .await;
        let travel = fact(
            &pool,
            napoleon,
            "travel",
            "Passage through Milan",
            json!({"kind":"exact","precision":"year","start":"1796"}),
            Some("1796-01-01"),
            Some("Milan"),
            None,
            "attested",
            None,
            None,
        )
        .await;
        let diplomatic = fact(
            &pool,
            napoleon,
            "diplomatic",
            "Armistice of Cherasco",
            json!({"kind":"exact","precision":"year","start":"1796"}),
            Some("1796-01-01"),
            Some("Cherasco"),
            Some((2.35, 48.86)),
            "uncertain",
            Some("city"),
            Some(5000.0),
        )
        .await;
        let draft_fact = fact(
            &pool,
            napoleon,
            "residence",
            "Unpublished note",
            json!({"kind":"approx","precision":"year","start":"1797"}),
            Some("1797-01-01"),
            None,
            None,
            "uncertain",
            None,
            None,
        )
        .await;
        let campaign = chapter(
            &pool,
            napoleon,
            "italian-campaign",
            "Campagne d'Italie",
            "chronological",
            Some("The 1796 campaign in northern Italy."),
            Some("editorial introduction"),
            0,
            false,
        )
        .await;
        let before: String =
            sqlx::query_scalar("SELECT epistemic_status FROM canonical_events WHERE id = $1")
                .bind(diplomatic.canonical)
                .fetch_one(&pool)
                .await
                .unwrap();
        talaria_store::set_editorial_group_status(&pool, campaign, talaria_store::STATUS_PUBLISHED)
            .await
            .unwrap();
        let after: String =
            sqlx::query_scalar("SELECT epistemic_status FROM canonical_events WHERE id = $1")
                .bind(diplomatic.canonical)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(before, after);
        assert_eq!(after, "uncertain");
        let northern = chapter(
            &pool,
            napoleon,
            "northern-italy",
            "Italie du Nord",
            "thematic",
            None,
            None,
            1,
            true,
        )
        .await;
        let letters = chapter(
            &pool,
            napoleon,
            "letters",
            "Lettres",
            "documentary",
            Some("No documented events in this chapter yet."),
            None,
            2,
            true,
        )
        .await;
        let private_notes = chapter(
            &pool,
            napoleon,
            "private-notes",
            "Notes privées",
            "narrative",
            Some("Draft"),
            None,
            3,
            false,
        )
        .await;
        for (group, fact_id, role, order) in [
            (campaign, diplomatic.logical, "primary", 0),
            (campaign, battle.logical, "primary", 1),
            (campaign, travel.logical, "supporting", 2),
            (northern, battle.logical, "primary", 0),
            (private_notes, draft_fact.logical, "context", 0),
        ] {
            talaria_store::add_editorial_membership(&pool, group, fact_id, role, order, None)
                .await
                .unwrap();
        }
        let successor = Uuid::new_v4();
        sqlx::query("UPDATE canonical_events SET is_active = false WHERE id = $1")
            .bind(battle.canonical)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            r#"
            INSERT INTO canonical_events (
                id, entity_id, event_type, title, start_time, time_json, pipeline, is_active,
                timeline_eligible, map_eligible, epistemic_status, logical_event_id,
                place_label, coord_precision_level, uncertainty_radius_m, geom, supersedes
            )
            SELECT $1, entity_id, event_type, 'Battle of Lodi, revised', start_time, time_json,
                   pipeline, true, timeline_eligible, map_eligible, epistemic_status,
                   logical_event_id, place_label, coord_precision_level, uncertainty_radius_m,
                   geom, $2
            FROM canonical_events WHERE id = $2
            "#,
        )
        .bind(successor)
        .bind(battle.canonical)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("UPDATE canonical_events SET superseded_by = $2 WHERE id = $1")
            .bind(battle.canonical)
            .bind(successor)
            .execute(&pool)
            .await
            .unwrap();

        let residence = fact(
            &pool,
            hugo,
            "residence",
            "Residence in Guernsey",
            json!({"kind":"range","precision":"year","start":"1855","end":"1870"}),
            Some("1855-01-01"),
            Some("Guernsey"),
            Some((-2.58, 49.45)),
            "attested",
            Some("point"),
            None,
        )
        .await;
        let publication = fact(
            &pool,
            hugo,
            "publication",
            "Les Misérables",
            json!({"kind":"approx","precision":"year","start":"1862"}),
            Some("1862-01-01"),
            None,
            None,
            "attested",
            None,
            None,
        )
        .await;
        let exile = chapter(
            &pool,
            hugo,
            "exile",
            "Exil",
            "narrative",
            Some("The years away from France."),
            Some("editorial introduction"),
            0,
            true,
        )
        .await;
        talaria_store::add_editorial_membership(
            &pool,
            exile,
            residence.logical,
            "primary",
            0,
            None,
        )
        .await
        .unwrap();
        talaria_store::add_editorial_membership(
            &pool,
            exile,
            publication.logical,
            "supporting",
            1,
            None,
        )
        .await
        .unwrap();
        let discovery = fact(
            &pool,
            curie,
            "publication",
            "Research on radioactivity",
            json!({"kind":"exact","precision":"year","start":"1898"}),
            Some("1898-01-01"),
            None,
            None,
            "uncertain",
            None,
            None,
        )
        .await;
        let research = chapter(
            &pool,
            curie,
            "radioactivity",
            "Radioactivité",
            "thematic",
            Some("Scientific publications."),
            None,
            0,
            true,
        )
        .await;
        talaria_store::add_editorial_membership(
            &pool,
            research,
            discovery.logical,
            "primary",
            0,
            None,
        )
        .await
        .unwrap();
        let lyceum = fact(
            &pool,
            aristotle,
            "education",
            "Teaching at the Lyceum",
            json!({"kind":"unknown"}),
            None,
            None,
            None,
            "uncertain",
            None,
            None,
        )
        .await;
        let school = chapter(
            &pool,
            aristotle,
            "lyceum",
            "Le Lycée",
            "thematic",
            Some("Teaching, without a modern exact date."),
            None,
            0,
            true,
        )
        .await;
        talaria_store::add_editorial_membership(&pool, school, lyceum.logical, "primary", 0, None)
            .await
            .unwrap();
        let memoir: Uuid = sqlx::query_scalar(
            "INSERT INTO raw_documents (source_type, source_uri, title) VALUES ('fixture', 'urn:v34:memoir', 'A memoir') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let other_book: Uuid = sqlx::query_scalar(
            "INSERT INTO raw_documents (source_type, source_uri, title) VALUES ('fixture', 'urn:v34:other', 'Another book') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO event_evidence (canonical_event_id, raw_document_id, quoted_text, evidence_hash)
            VALUES ($1, $2, 'first passage', 'passage-1'), ($1, $2, 'second passage', 'passage-2')
            "#,
        )
        .bind(successor)
        .bind(memoir)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO event_evidence (canonical_event_id, raw_document_id, quoted_text, evidence_hash)
            VALUES ($1, $2, 'archived passage', 'archived-passage')
            "#,
        )
        .bind(battle.canonical)
        .bind(other_book)
        .execute(&pool)
        .await
        .unwrap();
        let _ = letters;
        let app = state(pool);

        let (status, listed) = json_status(
            super::super::editorial::list_groups(
                State(app.clone()),
                Path(napoleon),
                Query(super::super::editorial::PageQuery::default()),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let slugs: Vec<&str> = listed["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|group| group["slug"].as_str().unwrap())
            .collect();
        assert_eq!(slugs, vec!["italian-campaign", "northern-italy", "letters"]);
        assert!(!slugs.contains(&"private-notes"));

        let (status, detail) = json_status(
            super::super::editorial::group_detail(
                State(app.clone()),
                Path((napoleon, "italian-campaign".into())),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(detail["group"]["title"], "Campagne d'Italie");
        assert_eq!(detail["group"]["summary_origin"], "editorial introduction");
        assert_eq!(detail["coverage"]["members"], 3);
        assert_eq!(detail["coverage"]["established"], 0);
        assert_eq!(detail["coverage"]["attested"], 2);
        assert_eq!(detail["coverage"]["in_review"], 1);
        assert_eq!(detail["coverage"]["without_coordinates"], 1);
        assert_eq!(detail["coverage"]["time_not_exact"], 0);
        assert_eq!(detail["coverage"]["with_evidence"], 1);
        assert_eq!(detail["coverage"]["evidence_rows"], 2);
        assert_eq!(detail["coverage"]["independent_sources"], 1);
        assert!(detail["coverage"]["completeness_rate"].is_null());
        assert_eq!(detail["coverage"]["scope_defined"], false);

        for slug in ["private-notes", "missing-chapter"] {
            let (status, body) = json_status(
                super::super::editorial::group_detail(
                    State(app.clone()),
                    Path((napoleon, slug.into())),
                )
                .await,
            )
            .await;
            assert_eq!(status, StatusCode::NOT_FOUND);
            assert_eq!(body["error"], "group_not_found");
            let (status, body) = json_status(
                view(
                    app.clone(),
                    napoleon,
                    ViewQuery {
                        group: Some(slug.into()),
                        ..Default::default()
                    },
                    false,
                )
                .await,
            )
            .await;
            assert_eq!(status, StatusCode::NOT_FOUND);
            assert_eq!(body["error"], "group_not_found");
        }
        let (status, body) = json_status(
            view(
                app.clone(),
                napoleon,
                ViewQuery {
                    group: Some("Campagne".into()),
                    ..Default::default()
                },
                false,
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "invalid_group");

        let (status, empty) = json_status(
            view(
                app.clone(),
                napoleon,
                ViewQuery {
                    group: Some("letters".into()),
                    ..Default::default()
                },
                false,
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(empty["events"].as_array().unwrap().is_empty());

        let campaign_timeline = payload(
            view(
                app.clone(),
                napoleon,
                ViewQuery {
                    group: Some("italian-campaign".into()),
                    resolution: Some("overview".into()),
                    ..Default::default()
                },
                false,
            )
            .await,
        )
        .await;
        let campaign_ids = event_ids(&campaign_timeline);
        assert!(campaign_ids.contains(&successor.to_string()));
        assert!(campaign_ids.contains(&travel.canonical.to_string()));
        assert!(campaign_ids.contains(&diplomatic.canonical.to_string()));
        assert!(!campaign_ids.contains(&battle.canonical.to_string()));
        assert!(!campaign_ids.contains(&birth.canonical.to_string()));
        assert!(!campaign_ids.contains(&draft_fact.canonical.to_string()));
        let battle_item = campaign_timeline["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == successor.to_string())
            .unwrap();
        assert_eq!(battle_item["time"]["precision"], "day");
        assert_eq!(battle_item["time"]["start"], "1796-05-10");
        assert_eq!(battle_item["epistemic_status"], "attested");
        let place_item = campaign_timeline["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["event_type"] == "diplomatic")
            .unwrap();
        assert_eq!(place_item["epistemic_status"], "uncertain");
        assert_eq!(place_item["coord_precision_level"], "city");
        assert_eq!(place_item["uncertainty_radius_m"], 5000.0);
        let unlocated = campaign_timeline["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["event_type"] == "travel")
            .unwrap();
        assert!(unlocated["coordinates"].is_null());

        let general = payload(
            view(
                app.clone(),
                napoleon,
                ViewQuery {
                    resolution: Some("overview".into()),
                    ..Default::default()
                },
                false,
            )
            .await,
        )
        .await;
        let general_ids = event_ids(&general);
        assert!(general_ids.contains(&birth.canonical.to_string()));
        assert!(general_ids.contains(&successor.to_string()));
        assert!(!general_ids.contains(&travel.canonical.to_string()));

        let other_group = event_ids(
            &payload(
                view(
                    app.clone(),
                    napoleon,
                    ViewQuery {
                        group: Some("northern-italy".into()),
                        ..Default::default()
                    },
                    false,
                )
                .await,
            )
            .await,
        );
        assert_eq!(other_group, vec![successor.to_string()]);

        let filtered = payload(
            view(
                app.clone(),
                napoleon,
                ViewQuery {
                    group: Some("italian-campaign".into()),
                    from: Some(1796),
                    to: Some(1796),
                    bbox: Some("9.4,45.2,9.6,45.4".into()),
                    ..Default::default()
                },
                true,
            )
            .await,
        )
        .await;
        let features = filtered["features"].as_array().unwrap();
        assert_eq!(features.len(), 1);
        assert_eq!(features[0]["id"], successor.to_string());
        assert_eq!(features[0]["properties"]["coord_precision_level"], "point");
        let chapter_map = payload(
            view(
                app.clone(),
                napoleon,
                ViewQuery {
                    group: Some("italian-campaign".into()),
                    ..Default::default()
                },
                true,
            )
            .await,
        )
        .await;
        let diplomatic_place = chapter_map["features"]
            .as_array()
            .unwrap()
            .iter()
            .find(|feature| feature["properties"]["event_type"] == "diplomatic")
            .unwrap();
        assert_eq!(
            diplomatic_place["properties"]["coord_precision_level"],
            "city"
        );
        assert_eq!(
            diplomatic_place["properties"]["uncertainty_radius_m"],
            5000.0
        );
        assert!(chapter_map["features"]
            .as_array()
            .unwrap()
            .iter()
            .all(|feature| feature["id"] != battle.canonical.to_string()));

        let (status, ordered) = json_status(
            super::super::editorial::group_events(
                State(app.clone()),
                Path((napoleon, "italian-campaign".into())),
                Query(super::super::editorial::PageQuery {
                    limit: Some(1),
                    cursor: None,
                }),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(ordered["events"][0]["id"], diplomatic.canonical.to_string());
        assert_eq!(
            ordered["events"][0]["logical_event_id"],
            diplomatic.logical.to_string()
        );
        let next = ordered["pagination"]["next_cursor"]
            .as_str()
            .unwrap()
            .to_string();
        let (status, second) = json_status(
            super::super::editorial::group_events(
                State(app.clone()),
                Path((napoleon, "italian-campaign".into())),
                Query(super::super::editorial::PageQuery {
                    limit: Some(1),
                    cursor: Some(next),
                }),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(second["events"][0]["id"], successor.to_string());
        assert_eq!(
            second["events"][0]["logical_event_id"],
            battle.logical.to_string()
        );
        assert_eq!(second["events"][0]["editorial_role"], "primary");
        assert_eq!(second["events"][0]["evidence_count"], 2);
        assert_eq!(ordered["events"][0]["coord_precision_level"], "city");
        assert_eq!(ordered["events"][0]["uncertainty_radius_m"], 5000.0);

        let exile_timeline = payload(
            view(
                app.clone(),
                hugo,
                ViewQuery {
                    group: Some("exile".into()),
                    ..Default::default()
                },
                false,
            )
            .await,
        )
        .await;
        assert_eq!(event_ids(&exile_timeline).len(), 2);
        assert_eq!(exile_timeline["events"][1]["time"]["kind"], "approx");
        let exile_map = payload(
            view(
                app.clone(),
                hugo,
                ViewQuery {
                    group: Some("exile".into()),
                    ..Default::default()
                },
                true,
            )
            .await,
        )
        .await;
        assert_eq!(exile_map["features"].as_array().unwrap().len(), 1);
        assert_eq!(
            exile_map["features"][0]["id"],
            residence.canonical.to_string()
        );

        let curie_map = payload(
            view(
                app.clone(),
                curie,
                ViewQuery {
                    group: Some("radioactivity".into()),
                    ..Default::default()
                },
                true,
            )
            .await,
        )
        .await;
        assert!(curie_map["features"].as_array().unwrap().is_empty());
        let curie_timeline = payload(
            view(
                app.clone(),
                curie,
                ViewQuery {
                    group: Some("radioactivity".into()),
                    ..Default::default()
                },
                false,
            )
            .await,
        )
        .await;
        assert_eq!(curie_timeline["events"][0]["epistemic_status"], "uncertain");
        assert!(curie_timeline["events"][0]["coordinates"].is_null());

        let ancient = payload(
            view(
                app.clone(),
                aristotle,
                ViewQuery {
                    group: Some("lyceum".into()),
                    ..Default::default()
                },
                false,
            )
            .await,
        )
        .await;
        assert_eq!(ancient["events"][0]["time"]["kind"], "unknown");
        assert!(ancient["events"][0]["time"].get("start").is_none());
        let (status, ancient_detail) = json_status(
            super::super::editorial::group_detail(
                State(app.clone()),
                Path((aristotle, "lyceum".into())),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(ancient_detail["coverage"]["time_not_exact"], 1);
        assert_eq!(ancient_detail["coverage"]["without_coordinates"], 1);
        assert!(ancient_detail["coverage"]["completeness_rate"].is_null());
        let (status, hugo_detail) = json_status(
            super::super::editorial::group_detail(State(app.clone()), Path((hugo, "exile".into())))
                .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(hugo_detail["coverage"]["members"], 2);
        assert_eq!(hugo_detail["coverage"]["time_not_exact"], 2);
        assert_eq!(hugo_detail["coverage"]["without_coordinates"], 1);
        let (status, empty_detail) = json_status(
            super::super::editorial::group_detail(
                State(app.clone()),
                Path((napoleon, "letters".into())),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(empty_detail["coverage"]["members"], 0);
        assert!(empty_detail["coverage"]["completeness_rate"].is_null());
        let (status, hugo_groups) = json_status(
            super::super::editorial::list_groups(
                State(app),
                Path(hugo),
                Query(super::super::editorial::PageQuery::default()),
            )
            .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(hugo_groups["groups"][0]["slug"], "exile");
        assert_eq!(hugo_groups["groups"].as_array().unwrap().len(), 1);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn chapter_timeline_pages_beyond_two_hundred_events(pool: sqlx::PgPool) {
        let entity = person(&pool, "Long chapter fixture").await;
        let group = chapter(
            &pool,
            entity,
            "long-chapter",
            "Long chapter",
            "chronological",
            None,
            None,
            0,
            true,
        )
        .await;
        sqlx::query(
            r#"
            WITH created AS (
                INSERT INTO logical_events (origin_occurrence_key)
                SELECT 'moment-' || n::text FROM generate_series(1, 201) AS n
                RETURNING id
            )
            INSERT INTO canonical_events (
                entity_id, event_type, title, start_time, time_json, pipeline, is_active,
                timeline_eligible, map_eligible, logical_event_id
            )
            SELECT $1, 'other', id::text, '1800-01-01',
                   '{"kind":"exact","precision":"year","start":"1800"}',
                   'person', true, true, false, id
            FROM created
            "#,
        )
        .bind(entity)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO editorial_memberships (group_id, logical_event_id, editorial_role, display_order)
            SELECT $1, logical_event_id, 'context', 0
            FROM canonical_events
            WHERE entity_id = $2
            "#,
        )
        .bind(group)
        .bind(entity)
        .execute(&pool)
        .await
        .unwrap();
        let app = state(pool);
        let first = payload(
            view(
                app.clone(),
                entity,
                ViewQuery {
                    group: Some("long-chapter".into()),
                    limit: Some(200),
                    ..Default::default()
                },
                false,
            )
            .await,
        )
        .await;
        assert_eq!(first["events"].as_array().unwrap().len(), 200);
        let cursor = first["pagination"]["next_cursor"]
            .as_str()
            .unwrap()
            .to_string();
        let second = payload(
            view(
                app,
                entity,
                ViewQuery {
                    group: Some("long-chapter".into()),
                    limit: Some(200),
                    cursor: Some(cursor),
                    ..Default::default()
                },
                false,
            )
            .await,
        )
        .await;
        assert_eq!(second["events"].as_array().unwrap().len(), 1);
        assert!(second["pagination"]["next_cursor"].is_null());
        let mut seen = event_ids(&first);
        seen.extend(event_ids(&second));
        assert_eq!(seen.len(), 201);
        assert_eq!(
            seen.iter().collect::<std::collections::HashSet<_>>().len(),
            201
        );
    }
}
