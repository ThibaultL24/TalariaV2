// crates/talaria-api/src/routes/editorial.rs
//! Public chapter reads. Drafts use the same missing-chapter response.
//! Publication is not an endpoint here and does not change a fact's status.

use super::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use talaria_store::{
    chapter_coverage, find_published_editorial_group, is_editorial_slug,
    list_editorial_group_events, list_editorial_groups, ChapterCoverage, EditorialEventFilter,
    STATUS_PUBLISHED,
};
use uuid::Uuid;

const MAX_PAGE: i64 = 100;

#[derive(Debug, Deserialize, Default)]
pub struct PageQuery {
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GroupCursor {
    order: i32,
    id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
struct EventCursor {
    group: Uuid,
    order: i32,
    id: Uuid,
}

fn error(status: StatusCode, code: &str) -> Response {
    (status, Json(json!({"error": code}))).into_response()
}

fn page_limit(limit: Option<i64>) -> Result<i64, Response> {
    match limit {
        None => Ok(50),
        Some(limit) if (1..=MAX_PAGE).contains(&limit) => Ok(limit),
        Some(_) => Err(error(StatusCode::BAD_REQUEST, "invalid_limit")),
    }
}

fn decode_cursor<T: for<'de> Deserialize<'de>>(value: &str) -> Option<T> {
    if value.len() > 8192 {
        return None;
    }
    let bytes = hex::decode(value).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn encode_cursor(value: &impl Serialize) -> String {
    hex::encode(serde_json::to_vec(value).expect("cursor serialization"))
}

fn group_json(group: &talaria_store::EditorialGroup) -> Value {
    json!({
        "id": group.id,
        "slug": group.slug,
        "title": group.title,
        "group_kind": group.group_kind,
        "summary": group.summary,
        "summary_origin": group.summary_origin,
        "display_order": group.display_order,
    })
}

fn coverage_json(coverage: &ChapterCoverage) -> Value {
    json!({
        "members": coverage.members,
        "established": coverage.established,
        "attested": coverage.attested,
        "in_review": coverage.in_review,
        "without_coordinates": coverage.without_coordinates,
        "time_not_exact": coverage.time_not_exact,
        "with_evidence": coverage.with_evidence,
        "evidence_rows": coverage.evidence_rows,
        "independent_sources": coverage.independent_sources,
        "scope_defined": false,
        "completeness_rate": Value::Null,
    })
}

async fn require_entity(state: &AppState, entity: Uuid) -> Result<(), Response> {
    match talaria_store::get_entity(&state.pool, entity).await {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(error(StatusCode::NOT_FOUND, "entity_not_found")),
        Err(reason) => {
            tracing::error!(error = %reason, "editorial entity lookup failed");
            Err(error(StatusCode::INTERNAL_SERVER_ERROR, "database_error"))
        }
    }
}

pub async fn list_groups(
    State(state): State<AppState>,
    Path(entity): Path<Uuid>,
    Query(query): Query<PageQuery>,
) -> Response {
    let limit = match page_limit(query.limit) {
        Ok(limit) => limit,
        Err(response) => return response,
    };
    let cursor = match query.cursor.as_deref() {
        None => None,
        Some(value) => match decode_cursor::<GroupCursor>(value) {
            Some(cursor) => Some(cursor),
            None => return error(StatusCode::BAD_REQUEST, "invalid_cursor"),
        },
    };
    if let Err(response) = require_entity(&state, entity).await {
        return response;
    }
    match list_editorial_groups(
        &state.pool,
        entity,
        Some(STATUS_PUBLISHED),
        limit,
        cursor.as_ref().map(|item| item.order),
        cursor.as_ref().map(|item| item.id),
    )
    .await
    {
        Ok(page) => {
            let next = page.has_more.then(|| {
                let last = page.groups.last().expect("a full page has a last chapter");
                encode_cursor(&GroupCursor {
                    order: last.display_order,
                    id: last.id,
                })
            });
            Json(json!({
                "groups": page.groups.iter().map(group_json).collect::<Vec<_>>(),
                "pagination": { "next_cursor": next },
            }))
            .into_response()
        }
        Err(reason) => {
            tracing::error!(error = %reason, "editorial group list failed");
            error(StatusCode::INTERNAL_SERVER_ERROR, "database_error")
        }
    }
}

pub async fn group_detail(
    State(state): State<AppState>,
    Path((entity, slug)): Path<(Uuid, String)>,
) -> Response {
    if !is_editorial_slug(&slug) {
        return error(StatusCode::BAD_REQUEST, "invalid_group");
    }
    if let Err(response) = require_entity(&state, entity).await {
        return response;
    }
    match find_published_editorial_group(&state.pool, entity, &slug).await {
        Ok(Some(group)) => match chapter_coverage(&state.pool, group.id).await {
            Ok(coverage) => Json(json!({
                "group": group_json(&group),
                "coverage": coverage_json(&coverage),
            }))
            .into_response(),
            Err(reason) => {
                tracing::error!(error = %reason, "editorial coverage failed");
                error(StatusCode::INTERNAL_SERVER_ERROR, "database_error")
            }
        },
        Ok(None) => error(StatusCode::NOT_FOUND, "group_not_found"),
        Err(reason) => {
            tracing::error!(error = %reason, "editorial group detail failed");
            error(StatusCode::INTERNAL_SERVER_ERROR, "database_error")
        }
    }
}

pub async fn group_events(
    State(state): State<AppState>,
    Path((entity, slug)): Path<(Uuid, String)>,
    Query(query): Query<PageQuery>,
) -> Response {
    if !is_editorial_slug(&slug) {
        return error(StatusCode::BAD_REQUEST, "invalid_group");
    }
    let limit = match page_limit(query.limit) {
        Ok(limit) => limit,
        Err(response) => return response,
    };
    if let Err(response) = require_entity(&state, entity).await {
        return response;
    }
    let group = match find_published_editorial_group(&state.pool, entity, &slug).await {
        Ok(Some(group)) => group,
        Ok(None) => return error(StatusCode::NOT_FOUND, "group_not_found"),
        Err(reason) => {
            tracing::error!(error = %reason, "editorial group events failed");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "database_error");
        }
    };
    let cursor = match query.cursor.as_deref() {
        None => None,
        Some(value) => match decode_cursor::<EventCursor>(value) {
            Some(cursor) if cursor.group == group.id => Some(cursor),
            Some(_) | None => return error(StatusCode::BAD_REQUEST, "invalid_cursor"),
        },
    };
    match list_editorial_group_events(
        &state.pool,
        group.id,
        EditorialEventFilter {
            event_type: None,
            epistemic_status: None,
            map_eligible: None,
            timeline_eligible: None,
        },
        limit,
        cursor.as_ref().map(|item| item.order),
        cursor.as_ref().map(|item| item.id),
    )
    .await
    {
        Ok(page) => {
            let next = page.has_more.then(|| {
                let last = page.events.last().expect("a full page has a last event");
                encode_cursor(&EventCursor {
                    group: group.id,
                    order: last.display_order,
                    id: last.membership_id,
                })
            });
            let events: Vec<Value> = page
                .events
                .iter()
                .map(|event| {
                    json!({
                        "id": event.canonical_event_id,
                        "logical_event_id": event.logical_event_id,
                        "editorial_role": event.editorial_role,
                        "display_order": event.display_order,
                        "event_type": event.event_type,
                        "epistemic_status": event.epistemic_status,
                        "title": event.title,
                        "time": event.time_json,
                        "place_label": event.place_label,
                        "map_eligible": event.map_eligible,
                        "coord_precision_level": event.coord_precision_level,
                        "uncertainty_radius_m": event.uncertainty_radius_m,
                        "evidence_count": event.evidence_count,
                        "coordinates": match (event.lat, event.lon) {
                            (Some(lat), Some(lon)) => json!({"lat": lat, "lon": lon}),
                            _ => Value::Null,
                        },
                    })
                })
                .collect();
            Json(json!({
                "events": events,
                "pagination": { "next_cursor": next },
            }))
            .into_response()
        }
        Err(reason) => {
            tracing::error!(error = %reason, "editorial group events failed");
            error(StatusCode::INTERNAL_SERVER_ERROR, "database_error")
        }
    }
}

/// Computed groupings. Nothing is written, and nothing here is a published chapter.
pub async fn chapter_suggestions(
    State(state): State<AppState>,
    Path(entity): Path<Uuid>,
) -> Response {
    if let Err(response) = require_entity(&state, entity).await {
        return response;
    }
    let rows: Result<Vec<(String, i64)>, sqlx::Error> = sqlx::query_as(
        "SELECT event_type, count(*)::bigint \
         FROM canonical_events \
         WHERE entity_id = $1 AND pipeline = 'person' AND is_active \
         GROUP BY event_type",
    )
    .bind(entity)
    .fetch_all(&state.pool)
    .await;
    let rows = match rows {
        Ok(rows) => rows,
        Err(reason) => {
            tracing::error!(error = %reason, "chapter suggestion counts failed");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "database_error");
        }
    };
    let counts: Vec<(&str, usize)> = rows
        .iter()
        .map(|(kind, count)| (kind.as_str(), *count as usize))
        .collect();
    let suggestions = talaria_quality::suggest_chapters(&counts);
    let moments: Result<Vec<(i32, Option<String>)>, sqlx::Error> = sqlx::query_as(
        "SELECT EXTRACT(YEAR FROM start_time)::int, place_label \
         FROM canonical_events \
         WHERE entity_id = $1 AND pipeline = 'person' AND is_active AND start_time IS NOT NULL \
         ORDER BY start_time \
         LIMIT 800",
    )
    .bind(entity)
    .fetch_all(&state.pool)
    .await;
    let moments = match moments {
        Ok(moments) => moments,
        Err(reason) => {
            tracing::error!(error = %reason, "chapter period dates failed");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "database_error");
        }
    };
    let facts: Vec<talaria_quality::DatedFact> = moments
        .into_iter()
        .map(|(year, place)| talaria_quality::DatedFact { year, place })
        .collect();
    let periods = talaria_quality::suggest_life_periods(&facts);
    let period_body: Vec<Value> = periods
        .iter()
        .map(|item| {
            json!({
                "slug": format!("period-{}-{}", item.start_year, item.end_year),
                "title_fr": item.title_fr,
                "title_en": item.title_en,
                "event_count": item.event_count,
                "standing": talaria_quality::SUGGESTION,
                "basis_fr": item.basis_fr,
                "basis_en": item.basis_en,
                "start_year": item.start_year,
                "end_year": item.end_year,
                "members": item.members.len(),
            })
        })
        .collect();
    let body: Vec<Value> = suggestions
        .iter()
        .map(|item| {
            json!({
                "slug": item.slug,
                "title_fr": item.title_fr,
                "title_en": item.title_en,
                "event_count": item.event_count,
                "standing": item.standing,
                "basis_fr": item.basis_fr,
                "basis_en": item.basis_en,
            })
        })
        .collect();
    let located: Result<Vec<(String, Option<String>)>, sqlx::Error> = sqlx::query_as(
        "SELECT title, place_label \
         FROM canonical_events \
         WHERE entity_id = $1 AND pipeline = 'person' AND is_active \
         ORDER BY start_time NULLS LAST \
         LIMIT 800",
    )
    .bind(entity)
    .fetch_all(&state.pool)
    .await;
    let located = match located {
        Ok(located) => located,
        Err(reason) => {
            tracing::error!(error = %reason, "chapter place rows failed");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "database_error");
        }
    };
    let mut place_members: Vec<(String, Vec<String>)> = Vec::new();
    for (title, place) in located {
        let Some(place) = place.filter(|label| !label.trim().is_empty()) else {
            continue;
        };
        if let Some((_, members)) = place_members.iter_mut().find(|(name, _)| name == &place) {
            members.push(title);
        } else {
            place_members.push((place, vec![title]));
        }
    }
    let place_refs: Vec<(&str, Vec<String>)> = place_members
        .iter()
        .map(|(place, members)| (place.as_str(), members.clone()))
        .collect();
    let places = talaria_quality::suggest_place_chapters(&place_refs);
    let traced: Result<Vec<(String, String)>, sqlx::Error> = sqlx::query_as(
        "SELECT ce.title, ee.source_locator \
         FROM canonical_events ce \
         JOIN event_evidence ee ON ee.canonical_event_id = ce.id \
         WHERE ce.entity_id = $1 AND ce.pipeline = 'person' AND ce.is_active \
           AND ee.source_locator LIKE '%\"section\"%' \
         LIMIT 1500",
    )
    .bind(entity)
    .fetch_all(&state.pool)
    .await;
    let traced = match traced {
        Ok(traced) => traced,
        Err(reason) => {
            tracing::error!(error = %reason, "chapter section rows failed");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "database_error");
        }
    };
    let section_facts: Vec<talaria_quality::SectionFact> = traced
        .iter()
        .filter_map(|(title, locator)| {
            let section = serde_json::from_str::<Value>(locator)
                .ok()
                .and_then(|value| value.get("section")?.as_str().map(str::to_string))?;
            Some(talaria_quality::SectionFact {
                section,
                event_title: title.clone(),
            })
        })
        .collect();
    let sections = talaria_quality::suggest_section_chapters(&section_facts);
    Json(json!({
        "standing": talaria_quality::SUGGESTION,
        "published": false,
        "method": "Activities still need six facts of one type. Sections and places use the admitted facts themselves.",
        "suggestions": body,
        "periods": period_body,
        "sections": named_body(&sections),
        "places": named_body(&places),
    }))
    .into_response()
}

fn named_body(chapters: &[talaria_quality::NamedChapter]) -> Vec<Value> {
    chapters
        .iter()
        .map(|item| {
            json!({
                "slug": item.slug,
                "title_fr": item.title,
                "title_en": item.title,
                "event_count": item.event_count,
                "standing": talaria_quality::SUGGESTION,
                "basis_fr": item.basis_fr,
                "basis_en": item.basis_en,
                "members": item.members,
            })
        })
        .collect()
}
