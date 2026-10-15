// crates/talaria-api/src/routes/visit.rs
//! Visit HTTP surface. It reads biography and writes only heritage tables.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use talaria_store::{
    activity_status, catalog_heritage, commemoration_status, insert_activity, insert_commemoration, insert_itinerary,
    list_activities, list_commemorations, list_heritage_sites, load_site_points, order_stops,
    persist_catalog, project_lived_heritage, require_provenance, ActivityRow, CatalogReport, ClaimDirection,
    CommonsCandidate, CommemorationRow, HeritageSiteRow, MerimeeHit, PlaceClaim, PlaceProfile,
    ITINERARY_GENERATION_VERSION,
};
use uuid::Uuid;

use super::AppState;

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub category: Option<String>,
    pub bbox: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct DatedBody {
    pub title: String,
    pub kind: Option<String>,
    pub site_id: Option<Uuid>,
    pub year: Option<i32>,
    pub future_verified: Option<bool>,
    pub place_extant: Option<bool>,
    pub provenance: Value,
}

#[derive(Debug, Deserialize, Default)]
pub struct DiscoverBody {
    #[serde(default)]
    pub live: bool,
    #[serde(default)]
    pub person_label: Option<String>,
    #[serde(default)]
    pub profiles: Vec<PlaceProfile>,
    #[serde(default)]
    pub claims: Vec<PlaceClaim>,
    #[serde(default)]
    pub merimee: Vec<MerimeeHit>,
    #[serde(default)]
    pub media: Vec<CommonsCandidate>,
}

#[derive(Debug, Deserialize)]
pub struct ItineraryBody {
    pub entity_id: String,
    pub mode: String,
    pub origin_lat: Option<f64>,
    pub origin_lon: Option<f64>,
    pub site_ids: Option<Vec<Uuid>>,
}

fn error(status: StatusCode, code: &str) -> Response {
    (status, Json(json!({ "error": code }))).into_response()
}

fn page(limit: Option<i64>, offset: Option<i64>) -> Result<(i64, i64), Response> {
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    if !(1..=100).contains(&limit) || offset < 0 || offset > 10_000 {
        return Err(error(StatusCode::BAD_REQUEST, "invalid_page"));
    }
    Ok((limit, offset))
}

fn parse_bbox(value: Option<&str>) -> Result<Option<[f64; 4]>, Response> {
    let Some(value) = value else {
        return Ok(None);
    };
    let numbers: Vec<f64> = value
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| error(StatusCode::BAD_REQUEST, "invalid_bbox"))?;
    if numbers.len() != 4 || numbers.iter().any(|n| !n.is_finite()) {
        return Err(error(StatusCode::BAD_REQUEST, "invalid_bbox"));
    }
    let [west, south, east, north] = [numbers[0], numbers[1], numbers[2], numbers[3]];
    if !(-180.0..=180.0).contains(&west)
        || !(-180.0..=180.0).contains(&east)
        || !(-90.0..=90.0).contains(&south)
        || !(-90.0..=90.0).contains(&north)
        || west > east
        || south > north
    {
        return Err(error(StatusCode::BAD_REQUEST, "invalid_bbox"));
    }
    Ok(Some([west, south, east, north]))
}

async fn resolve_entity(state: &AppState, key: &str) -> Result<talaria_store::EntityRow, Response> {
    let key = key.trim();
    let found = if key.starts_with('Q') && key[1..].chars().all(|c| c.is_ascii_digit()) {
        talaria_store::find_entity_by_qid(&state.pool, key).await
    } else {
        let id = Uuid::parse_str(key).map_err(|_| error(StatusCode::BAD_REQUEST, "invalid_entity"))?;
        talaria_store::get_entity(&state.pool, id).await
    };
    match found {
        Ok(Some(entity)) => Ok(entity),
        Ok(None) => Err(error(StatusCode::NOT_FOUND, "entity_not_found")),
        Err(err) => {
            tracing::error!(error = %err, "visit entity lookup failed");
            Err(error(StatusCode::INTERNAL_SERVER_ERROR, "database_error"))
        }
    }
}

fn site_json(row: &HeritageSiteRow) -> Value {
    json!({
        "id": row.id,
        "place_label": row.place_label,
        "place_qid": row.place_qid,
        "category": row.category,
        "relation": row.relation,
        "temporal_class": row.temporal_class,
        "coordinates": match (row.lat, row.lon) {
            (Some(lat), Some(lon)) => json!({ "lat": lat, "lon": lon }),
            _ => Value::Null,
        },
        "visitability": row.visitability,
        "official_url": row.official_url,
        "practical": row.practical_json,
        "practical_verified_at": row.practical_verified_at,
        "summary": row.summary,
        "provenance": row.provenance_json,
        "image": match (&row.image_thumb_url, &row.image_credit, &row.image_license) {
            (Some(url), Some(credit), Some(license)) => json!({
                "thumb_url": url,
                "credit": credit,
                "license": license,
                "commons_file": row.image_commons_file,
            }),
            _ => Value::Null,
        },
        "merimee": match &row.merimee_ref {
            Some(reference) => json!({
                "reference": reference,
                "protection_kind": row.merimee_protection_kind,
                "commune": row.merimee_commune,
                "historique": row.merimee_historique,
                "source": "merimee_local",
            }),
            None => Value::Null,
        },
    })
}

fn page_json(items: Vec<Value>, limit: i64, offset: i64) -> Value {
    let next_offset = if items.len() as i64 == limit {
        Some(offset + limit)
    } else {
        None
    };
    json!({ "items": items, "limit": limit, "offset": offset, "next_offset": next_offset })
}

pub async fn by_qid(State(state): State<AppState>, Path(qid): Path<String>) -> Response {
    match resolve_entity(&state, &qid).await {
        Ok(entity) => Json(json!({
            "entity_id": entity.id,
            "qid": entity.qid,
            "label": entity.canonical_name.unwrap_or(entity.wikipedia_title),
        }))
        .into_response(),
        Err(response) => response,
    }
}

pub async fn project(State(state): State<AppState>, Path(key): Path<String>) -> Response {
    let entity = match resolve_entity(&state, &key).await {
        Ok(entity) => entity,
        Err(response) => return response,
    };
    match project_lived_heritage(&state.pool, entity.id).await {
        Ok(linked) => Json(json!({
            "entity_id": entity.id,
            "linked": linked,
            "note": "Only place labels that name a building or monument are projected. A city pin is not a heritage site. Visitability stays unknown.",
        }))
        .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "visit project failed");
            error(StatusCode::INTERNAL_SERVER_ERROR, "database_error")
        }
    }
}

pub async fn discover(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(body): Json<DiscoverBody>,
) -> Response {
    let entity = match resolve_entity(&state, &key).await {
        Ok(entity) => entity,
        Err(response) => return response,
    };
    let label = body.person_label.clone().unwrap_or_else(|| {
        entity.canonical_name.clone().unwrap_or_else(|| entity.wikipedia_title.clone())
    });
    let (report, upstream_errors) = if body.live {
        let Some(qid) = entity.qid.clone() else {
            return error(StatusCode::BAD_REQUEST, "missing_qid");
        };
        match super::visit_upstream::fetch_catalog(&state.pool, &qid, &label).await {
            Ok(value) => value,
            Err(err) => {
                tracing::error!(error = %err, "visit discover upstream failed");
                return error(StatusCode::BAD_GATEWAY, "upstream_failed");
            }
        }
    } else {
        let qid = entity.qid.clone().unwrap_or_default();
        (catalog_heritage(&qid, &label, &body.profiles, &body.claims, &body.merimee, &body.media), Vec::new())
    };
    if let Err(err) = persist_catalog(&state.pool, entity.id, &report.accepted).await {
        tracing::error!(error = %err, "visit discover persist failed");
        return error(StatusCode::INTERNAL_SERVER_ERROR, "database_error");
    }
    Json(report_json(entity.id, &report, &upstream_errors)).into_response()
}

fn report_json(entity_id: Uuid, report: &CatalogReport, upstream_errors: &[String]) -> Value {
    let mut reasons = std::collections::BTreeMap::<String, usize>::new();
    for rejected in &report.rejected {
        *reasons.entry(rejected.reason.clone()).or_default() += 1;
    }
    json!({
        "entity_id": entity_id,
        "candidates": report.candidates,
        "accepted": report.accepted.len(),
        "geolocated": report.accepted.iter().filter(|site| site.lat.is_some() && site.lon.is_some()).count(),
        "with_photo": report.accepted.iter().filter(|site| site.media.is_some()).count(),
        "sourced_links": report.accepted.iter().map(|site| site.relations.len()).sum::<usize>(),
        "duplicates": report.duplicates,
        "rejected": reasons,
        "upstream_errors": upstream_errors,
        "visitability": "unknown",
        "practical": {},
    })
}

pub async fn sites(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(query): Query<PageQuery>,
) -> Response {
    let (limit, offset) = match page(query.limit, query.offset) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let bbox = match parse_bbox(query.bbox.as_deref()) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let entity = match resolve_entity(&state, &key).await {
        Ok(entity) => entity,
        Err(response) => return response,
    };
    let category = query.category.as_deref().filter(|value| !value.is_empty());
    match list_heritage_sites(&state.pool, entity.id, category, bbox, limit, offset).await {
        Ok(rows) => Json(page_json(rows.iter().map(site_json).collect(), limit, offset)).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "visit sites failed");
            error(StatusCode::INTERNAL_SERVER_ERROR, "database_error")
        }
    }
}

pub async fn map(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(query): Query<PageQuery>,
) -> Response {
    let bbox = match parse_bbox(query.bbox.as_deref()) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let entity = match resolve_entity(&state, &key).await {
        Ok(entity) => entity,
        Err(response) => return response,
    };
    let category = query.category.as_deref().filter(|value| !value.is_empty());
    match list_heritage_sites(&state.pool, entity.id, category, bbox, 500, 0).await {
        Ok(rows) => {
            let features: Vec<Value> = rows
                .iter()
                .filter_map(|row| {
                    let (lat, lon) = (row.lat?, row.lon?);
                    Some(json!({
                        "type": "Feature",
                        "id": row.id,
                        "geometry": { "type": "Point", "coordinates": [lon, lat] },
                        "properties": site_json(row),
                    }))
                })
                .collect();
            Json(json!({ "type": "FeatureCollection", "features": features })).into_response()
        }
        Err(err) => {
            tracing::error!(error = %err, "visit map failed");
            error(StatusCode::INTERNAL_SERVER_ERROR, "database_error")
        }
    }
}

pub async fn activities(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(query): Query<PageQuery>,
) -> Response {
    listed(state, key, query, true).await
}

pub async fn commemorations(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(query): Query<PageQuery>,
) -> Response {
    listed(state, key, query, false).await
}

async fn listed(state: AppState, key: String, query: PageQuery, activities: bool) -> Response {
    let (limit, offset) = match page(query.limit, query.offset) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let entity = match resolve_entity(&state, &key).await {
        Ok(entity) => entity,
        Err(response) => return response,
    };
    if activities {
        match list_activities(&state.pool, entity.id, limit, offset).await {
            Ok(rows) => Json(page_json(rows.iter().map(activity_json).collect(), limit, offset)).into_response(),
            Err(err) => {
                tracing::error!(error = %err, "visit activities failed");
                error(StatusCode::INTERNAL_SERVER_ERROR, "database_error")
            }
        }
    } else {
        match list_commemorations(&state.pool, entity.id, limit, offset).await {
            Ok(rows) => {
                Json(page_json(rows.iter().map(commemoration_json).collect(), limit, offset)).into_response()
            }
            Err(err) => {
                tracing::error!(error = %err, "visit commemorations failed");
                error(StatusCode::INTERNAL_SERVER_ERROR, "database_error")
            }
        }
    }
}

fn activity_json(row: &ActivityRow) -> Value {
    json!({
        "id": row.id,
        "site_id": row.site_id,
        "kind": row.kind,
        "title": row.title,
        "time": row.time_json,
        "occurrence_status": row.occurrence_status,
        "bookable": row.occurrence_status == "scheduled",
        "provenance": row.provenance_json,
    })
}

fn commemoration_json(row: &CommemorationRow) -> Value {
    json!({
        "id": row.id,
        "site_id": row.site_id,
        "title": row.title,
        "time": row.time_json,
        "occurrence_status": row.occurrence_status,
        "place_persistence": row.place_persistence,
        "bookable": false,
        "provenance": row.provenance_json,
    })
}

const ACTIVITY_KINDS: &[&str] = &[
    "exhibition",
    "conference",
    "reenactment",
    "festival",
    "guided_visit",
    "ceremony",
];

pub async fn create_activity(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(body): Json<DatedBody>,
) -> Response {
    let entity = match resolve_entity(&state, &key).await {
        Ok(entity) => entity,
        Err(response) => return response,
    };
    if require_provenance(&body.provenance).is_err() || body.title.trim().is_empty() {
        return error(StatusCode::BAD_REQUEST, "missing_provenance");
    }
    let kind = body.kind.as_deref().unwrap_or("exhibition");
    if !ACTIVITY_KINDS.contains(&kind) {
        return error(StatusCode::BAD_REQUEST, "invalid_kind");
    }
    let now_year = Utc::now().format("%Y").to_string().parse().unwrap_or(2026);
    let status = match activity_status(body.year, now_year) {
        Ok(status) => status,
        Err(_) => return error(StatusCode::BAD_REQUEST, "missing_year"),
    };
    let time = json!({ "kind": "exact", "precision": "year", "year": body.year });
    match insert_activity(
        &state.pool,
        entity.id,
        body.site_id,
        kind,
        body.title.trim(),
        time,
        status,
        body.provenance,
    )
    .await
    {
        Ok(id) => Json(json!({ "id": id, "occurrence_status": status, "bookable": status == "scheduled" })).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "visit activity insert failed");
            error(StatusCode::INTERNAL_SERVER_ERROR, "database_error")
        }
    }
}

pub async fn create_commemoration(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(body): Json<DatedBody>,
) -> Response {
    let entity = match resolve_entity(&state, &key).await {
        Ok(entity) => entity,
        Err(response) => return response,
    };
    if require_provenance(&body.provenance).is_err() || body.title.trim().is_empty() {
        return error(StatusCode::BAD_REQUEST, "missing_provenance");
    }
    let now_year = Utc::now().format("%Y").to_string().parse().unwrap_or(2026);
    let status = match commemoration_status(body.year, now_year, body.future_verified.unwrap_or(false)) {
        Ok(status) => status,
        Err(talaria_store::TemporalError::UnverifiedFuture) => {
            return error(StatusCode::BAD_REQUEST, "unverified_future")
        }
        Err(_) => return error(StatusCode::BAD_REQUEST, "missing_year"),
    };
    let persistence = if body.place_extant.unwrap_or(false) { "extant" } else { "unknown" };
    let time = json!({ "kind": "exact", "precision": "year", "year": body.year });
    match insert_commemoration(
        &state.pool,
        entity.id,
        body.site_id,
        body.title.trim(),
        time,
        status,
        persistence,
        body.provenance,
    )
    .await
    {
        Ok(id) => Json(json!({
            "id": id,
            "occurrence_status": status,
            "place_persistence": persistence,
            "bookable": false,
            "biography": false,
        }))
        .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "visit commemoration insert failed");
            error(StatusCode::INTERNAL_SERVER_ERROR, "database_error")
        }
    }
}

pub async fn create_itinerary(
    State(state): State<AppState>,
    Json(body): Json<ItineraryBody>,
) -> Response {
    if !matches!(body.mode.as_str(), "walk" | "bike" | "drive") {
        return error(StatusCode::BAD_REQUEST, "invalid_mode");
    }
    let origin = match (body.origin_lat, body.origin_lon) {
        (None, None) => None,
        (Some(lat), Some(lon)) if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) => {
            Some((lat, lon))
        }
        _ => return error(StatusCode::BAD_REQUEST, "invalid_origin"),
    };
    let entity = match resolve_entity(&state, &body.entity_id).await {
        Ok(entity) => entity,
        Err(response) => return response,
    };
    let ids = body.site_ids.unwrap_or_default();
    if ids.len() > 24 {
        return error(StatusCode::BAD_REQUEST, "too_many_stops");
    }
    let points = match load_site_points(&state.pool, entity.id, &ids).await {
        Ok(points) if !points.is_empty() => points,
        Ok(_) => return error(StatusCode::BAD_REQUEST, "no_sites"),
        Err(err) => {
            tracing::error!(error = %err, "visit itinerary sites failed");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "database_error");
        }
    };
    let start = origin.unwrap_or_else(|| (points[0].lat, points[0].lon));
    let ordered = order_stops(start, &points);
    match insert_itinerary(&state.pool, entity.id, &body.mode, origin, &ordered).await {
        Ok(id) => Json(json!({
            "id": id,
            "entity_id": entity.id,
            "mode": body.mode,
            "routing_status": "unavailable",
            "travel_minutes": Value::Null,
            "generation_version": ITINERARY_GENERATION_VERSION,
            "stops": ordered.iter().enumerate().map(|(ordinal, (site_id, km))| json!({
                "ordinal": ordinal,
                "site_id": site_id,
                "straight_line_km": km,
                "travel_minutes": Value::Null,
            })).collect::<Vec<_>>(),
            "note": "No routing engine is configured. Order uses straight-line distance only.",
        }))
        .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "visit itinerary insert failed");
            error(StatusCode::INTERNAL_SERVER_ERROR, "database_error")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::StatusCode;

    fn state(pool: sqlx::PgPool) -> AppState {
        AppState {
            pool,
            offline_only: true,
            config: talaria_core::AppConfig::from_env().unwrap(),
            ingest_jobs: std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    async fn json_body(response: Response) -> (StatusCode, Value) {
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1_000_000).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    fn house_claim() -> DiscoverBody {
        DiscoverBody {
            live: false,
            person_label: Some("Victor Hugo".into()),
            profiles: vec![
                PlaceProfile {
                    qid: "Q90".into(),
                    label: "Paris".into(),
                    instance_of: vec!["Q515".into()],
                    lat: Some(48.86),
                    lon: Some(2.35),
                    image_file: None,
                    revision_id: Some("1".into()),
                },
                PlaceProfile {
                    qid: "Q3129162".into(),
                    label: "Hauteville House".into(),
                    instance_of: vec!["Q3947".into()],
                    lat: Some(49.45),
                    lon: Some(-2.53),
                    image_file: None,
                    revision_id: Some("9".into()),
                },
            ],
            claims: vec![
                PlaceClaim {
                    pid: "P19".into(),
                    target_qid: "Q90".into(),
                    direction: ClaimDirection::Outgoing,
                },
                PlaceClaim {
                    pid: "P551".into(),
                    target_qid: "Q3129162".into(),
                    direction: ClaimDirection::Outgoing,
                },
            ],
            merimee: vec![],
            media: vec![CommonsCandidate {
                commons_file: "File:Hauteville.jpg".into(),
                depicts_qids: vec!["Q3129162".into()],
                author: Some("Example".into()),
                license: Some("CC BY-SA 4.0".into()),
                thumb_url: Some("https://example.test/hauteville.jpg".into()),
                site_qid: "Q3129162".into(),
                person_qid: "Q535".into(),
                is_site_p18: false,
            }],
        }
    }

    async fn insert_person(pool: &sqlx::PgPool, name: &str, qid: &str) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO entities (wikipedia_title, wiki_lang, canonical_name, qid) VALUES ($1, 'fr', $1, $2) RETURNING id",
        )
        .bind(name)
        .bind(qid)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn discover_persists_one_site_and_skips_the_city(pool: sqlx::PgPool) {
        let hugo = insert_person(&pool, "Victor Hugo", "Q535").await;
        let napoleon = insert_person(&pool, "Napoleon", "Q517").await;
        let app = state(pool.clone());
        let first = discover(State(app.clone()), Path(hugo.to_string()), Json(house_claim())).await;
        let (status, body) = json_body(first).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["accepted"], 1);
        assert_eq!(body["geolocated"], 1);
        assert_eq!(body["with_photo"], 1);
        assert_eq!(body["rejected"]["settlement_grain"], 1);
        assert_eq!(body["practical"], json!({}));

        let again = discover(State(app.clone()), Path(hugo.to_string()), Json(house_claim())).await;
        assert_eq!(json_body(again).await.0, StatusCode::OK);
        let site_rows: i64 = sqlx::query_scalar("SELECT count(*) FROM heritage_sites WHERE place_qid = 'Q3129162'")
            .fetch_one(&pool)
            .await
            .unwrap();
        let cities: i64 = sqlx::query_scalar("SELECT count(*) FROM heritage_sites WHERE place_qid = 'Q90'")
            .fetch_one(&pool)
            .await
            .unwrap();
        let hugo_links: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM entity_heritage_links WHERE entity_id = $1",
        )
        .bind(hugo)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(site_rows, 1);
        assert_eq!(cities, 0);
        assert_eq!(hugo_links, 1);

        let mut napoleon_body = house_claim();
        napoleon_body.person_label = Some("Napoleon".into());
        napoleon_body.media[0].person_qid = "Q517".into();
        discover(State(app.clone()), Path(napoleon.to_string()), Json(napoleon_body)).await;
        let shared: i64 = sqlx::query_scalar("SELECT count(*) FROM heritage_sites WHERE place_qid = 'Q3129162'")
            .fetch_one(&pool)
            .await
            .unwrap();
        let linked_people: i64 = sqlx::query_scalar(
            "SELECT count(DISTINCT entity_id) FROM entity_heritage_links l JOIN heritage_sites s ON s.id = l.site_id WHERE s.place_qid = 'Q3129162'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(shared, 1);
        assert_eq!(linked_people, 2);

        let listed = super::sites(
            State(app.clone()),
            Path(hugo.to_string()),
            Query(PageQuery { category: None, bbox: Some("-3,49,-2,50".into()), limit: Some(1), offset: Some(0) }),
        )
        .await;
        let (status, page) = json_body(listed).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(page["items"].as_array().unwrap().len(), 1);
        assert_eq!(page["items"][0]["visitability"], "unknown");
        assert_eq!(page["items"][0]["practical"], json!({}));
        assert_eq!(page["items"][0]["provenance"]["source"], "wikidata");
        assert!(page["items"][0]["image"]["thumb_url"].is_string());

        let outside = super::sites(
            State(app.clone()),
            Path(hugo.to_string()),
            Query(PageQuery { category: None, bbox: Some("10,0,12,1".into()), limit: None, offset: None }),
        )
        .await;
        assert_eq!(json_body(outside).await.1["items"].as_array().unwrap().len(), 0);

        let bad = super::sites(
            State(app.clone()),
            Path(hugo.to_string()),
            Query(PageQuery { category: None, bbox: Some("nope".into()), limit: None, offset: None }),
        )
        .await;
        assert_eq!(bad.status(), StatusCode::BAD_REQUEST);

        let site_id = page["items"][0]["id"].as_str().unwrap().to_string();
        let route = create_itinerary(
            State(app.clone()),
            Json(ItineraryBody {
                entity_id: hugo.to_string(),
                mode: "walk".into(),
                origin_lat: Some(49.46),
                origin_lon: Some(-2.54),
                site_ids: Some(vec![Uuid::parse_str(&site_id).unwrap()]),
            }),
        )
        .await;
        let (status, route_body) = json_body(route).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(route_body["routing_status"], "unavailable");
        assert!(route_body["travel_minutes"].is_null());
        assert!(route_body["stops"][0]["travel_minutes"].is_null());
        assert!(route_body["stops"][0]["straight_line_km"].as_f64().unwrap() >= 0.0);

        let before: i64 = sqlx::query_scalar("SELECT count(*) FROM canonical_events WHERE entity_id = $1")
            .bind(hugo)
            .fetch_one(&pool)
            .await
            .unwrap();
        let homage = create_commemoration(
            State(app),
            Path(hugo.to_string()),
            Json(DatedBody {
                title: "Hommage au Panthéon".into(),
                kind: None,
                site_id: None,
                year: Some(1885),
                future_verified: None,
                place_extant: Some(true),
                provenance: json!({"source": "wikidata", "pid": "P547"}),
            }),
        )
        .await;
        let (status, homage_body) = json_body(homage).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(homage_body["biography"], false);
        assert_eq!(homage_body["bookable"], false);
        let after: i64 = sqlx::query_scalar("SELECT count(*) FROM canonical_events WHERE entity_id = $1")
            .bind(hugo)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(before, after);
    }
}
