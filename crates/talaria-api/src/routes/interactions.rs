// crates/talaria-api/src/routes/interactions.rs
use super::auth::{optional_user, require_user};
use super::AppState;
use crate::auth::AuthError;
use crate::interactions::{
    create_interaction, delete_interaction, list_mine, parse_action, parse_target_type,
    parse_visibility, summarize_targets, InteractionDto, InteractionError, MAX_SUMMARY_IDS,
};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::CookieJar;
use serde::Deserialize;
use serde_json::{json, Value};
use talaria_store::{InteractionAction, InteractionTargetType, InteractionVisibility};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/interactions", post(post_interaction))
        .route("/api/v1/interactions/{id}", axum::routing::delete(delete_one))
        .route("/api/v1/me/interactions", get(list_my_interactions))
        .route("/api/v1/interactions/summary", get(summary))
}

fn interaction_error(err: InteractionError) -> (StatusCode, Json<Value>) {
    let status = match err {
        InteractionError::Unauthenticated => StatusCode::UNAUTHORIZED,
        InteractionError::TargetNotFound | InteractionError::InteractionNotFound => {
            StatusCode::NOT_FOUND
        }
        InteractionError::DatabaseError => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(json!({
            "error": { "code": err.code(), "message": err.message() }
        })),
    )
}

fn from_auth(err: AuthError) -> (StatusCode, Json<Value>) {
    super::auth::auth_error(err)
}

fn dto_json(dto: &InteractionDto) -> Value {
    json!({
        "id": dto.id,
        "action": dto.action,
        "target_type": dto.target_type,
        "target_id": dto.target_id,
        "visibility": dto.visibility,
        "created_at": dto.created_at,
        "updated_at": dto.updated_at
    })
}

#[derive(Deserialize)]
pub struct PostBody {
    action: String,
    target_type: String,
    target_id: Uuid,
    visibility: Option<String>,
}

async fn post_interaction(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<PostBody>,
) -> impl IntoResponse {
    let user = match require_user(&state.pool, &jar).await {
        Ok(u) => u,
        Err(err) => return from_auth(err).into_response(),
    };
    let action = match parse_action(&body.action) {
        Ok(a) => a,
        Err(err) => return interaction_error(err).into_response(),
    };
    let target_type = match parse_target_type(&body.target_type) {
        Ok(t) => t,
        Err(err) => return interaction_error(err).into_response(),
    };
    let visibility = match parse_visibility(body.visibility.as_deref()) {
        Ok(v) => v,
        Err(err) => return interaction_error(err).into_response(),
    };
    match create_interaction(
        &state.pool,
        user.id,
        action,
        target_type,
        body.target_id,
        visibility,
    )
    .await
    {
        Ok(result) => {
            let dto = match InteractionDto::try_from(result.row) {
                Ok(d) => d,
                Err(err) => return interaction_error(err).into_response(),
            };
            let status = if result.created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            };
            (status, Json(json!({ "interaction": dto_json(&dto) }))).into_response()
        }
        Err(err) => interaction_error(err).into_response(),
    }
}

async fn delete_one(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let user = match require_user(&state.pool, &jar).await {
        Ok(u) => u,
        Err(err) => return from_auth(err).into_response(),
    };
    match delete_interaction(&state.pool, user.id, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => interaction_error(err).into_response(),
    }
}

#[derive(Deserialize)]
pub struct ListQuery {
    action: Option<String>,
    target_type: Option<String>,
    cursor: Option<String>,
    limit: Option<i64>,
}

async fn list_my_interactions(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let user = match require_user(&state.pool, &jar).await {
        Ok(u) => u,
        Err(err) => return from_auth(err).into_response(),
    };
    let action = match q.action.as_deref() {
        None => None,
        Some(raw) => match parse_action(raw) {
            Ok(a) => Some(a),
            Err(err) => return interaction_error(err).into_response(),
        },
    };
    let target_type = match q.target_type.as_deref() {
        None => None,
        Some(raw) => match parse_target_type(raw) {
            Ok(t) => Some(t),
            Err(err) => return interaction_error(err).into_response(),
        },
    };
    match list_mine(
        &state.pool,
        user.id,
        action,
        target_type,
        q.cursor.as_deref(),
        q.limit,
    )
    .await
    {
        Ok((items, next_cursor)) => Json(json!({
            "items": items.iter().map(dto_json).collect::<Vec<_>>(),
            "next_cursor": next_cursor
        }))
        .into_response(),
        Err(err) => interaction_error(err).into_response(),
    }
}

#[derive(Deserialize)]
pub struct SummaryQuery {
    target_type: String,
    target_ids: String,
}

fn parse_target_ids(raw: &str) -> Result<Vec<Uuid>, InteractionError> {
    let ids: Vec<Uuid> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(Uuid::parse_str)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| InteractionError::TargetNotFound)?;
    if ids.len() > MAX_SUMMARY_IDS {
        return Err(InteractionError::BatchTooLarge);
    }
    Ok(ids)
}

async fn summary(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<SummaryQuery>,
) -> impl IntoResponse {
    let target_type = match parse_target_type(&q.target_type) {
        Ok(t) => t,
        Err(err) => return interaction_error(err).into_response(),
    };
    let ids = match parse_target_ids(&q.target_ids) {
        Ok(ids) => ids,
        Err(err) => return interaction_error(err).into_response(),
    };
    let user_id = optional_user(&state.pool, &jar).await.map(|u| u.id);
    match summarize_targets(&state.pool, target_type, ids, user_id).await {
        Ok(summaries) => Json(json!({ "summaries": summaries })).into_response(),
        Err(err) => interaction_error(err).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{SESSION_COOKIE, SESSION_TTL_DAYS};
    use crate::interactions::{create_interaction, InteractionError};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum_extra::extract::cookie::Cookie;
    use talaria_store::{
        hash_session_token, insert_session, resolve_or_create_evm_user, upsert_entity_with_kind,
        CanonicalEventInsert, ClaimInsert, CorpusDocumentInsert, NormalizedEvmAddress,
    };
    use tower::ServiceExt;

    fn dummy_config() -> talaria_core::AppConfig {
        talaria_core::AppConfig {
            database_url: "postgres://unused".into(),
            data_root: ".".into(),
            bind_addr: "127.0.0.1:0".into(),
            wiki_lang: "en".into(),
            cosmos_python: "python3".into(),
            cosmos_script: ".".into(),
            cosmos_batch_script: ".".into(),
            offline_only: true,
        }
    }

    fn state(pool: sqlx::PgPool) -> AppState {
        AppState {
            pool,
            offline_only: true,
            config: dummy_config(),
            ingest_jobs: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
        }
    }

    async fn session_cookie(pool: &sqlx::PgPool, hex: &str) -> (Uuid, Cookie<'static>) {
        let user = resolve_or_create_evm_user(pool, &NormalizedEvmAddress::parse(hex).unwrap())
            .await
            .unwrap();
        let token = format!("tok-{}", user.id);
        insert_session(
            pool,
            user.id,
            &hash_session_token(&token),
            chrono::Utc::now() + chrono::Duration::days(SESSION_TTL_DAYS),
        )
        .await
        .unwrap();
        let mut cookie = Cookie::new(SESSION_COOKIE, token);
        cookie.set_path("/");
        (user.id, cookie)
    }

    fn app(state: AppState) -> Router {
        Router::new().merge(router()).with_state(state)
    }

    async fn seed_person(pool: &sqlx::PgPool) -> Uuid {
        upsert_entity_with_kind(pool, "en", "PR3 Person", "person")
            .await
            .unwrap()
    }

    async fn seed_place(pool: &sqlx::PgPool) -> Uuid {
        upsert_entity_with_kind(pool, "en", "PR3 Place", "place")
            .await
            .unwrap()
    }

    async fn seed_claim(pool: &sqlx::PgPool, entity_id: Uuid) -> Uuid {
        talaria_store::insert_claim(
            pool,
            &ClaimInsert {
                entity_id,
                claim_kind: "theory".into(),
                text: "PR3 claim".into(),
                epistemic_status: "attested".into(),
                relation_to_subject: "direct".into(),
                event_time: None,
                place_label: None,
                confidence: 0.5,
                canonical_event_id: None,
                debate_type: None,
                evidence_layer: None,
            },
        )
        .await
        .unwrap()
    }

    async fn seed_event(pool: &sqlx::PgPool, entity_id: Uuid) -> Uuid {
        talaria_store::insert_canonical_event(
            pool,
            &CanonicalEventInsert {
                entity_id,
                event_type: "birth".into(),
                epistemic_status: "attested".into(),
                title: "Born".into(),
                summary: None,
                start_time: None,
                time_json: serde_json::json!({"kind":"unknown"}),
                place_label: None,
                lat: None,
                lon: None,
                confidence: 0.5,
                map_eligible: false,
            },
        )
        .await
        .unwrap()
    }

    async fn seed_source(pool: &sqlx::PgPool) -> Uuid {
        talaria_store::upsert_corpus_document(
            pool,
            &CorpusDocumentInsert {
                source_kind: "openalex".into(),
                external_id: format!("pr3-{}", Uuid::new_v4()),
                canonical_url: None,
                document_type: "article".into(),
                title: "PR3 source".into(),
                language: Some("en".into()),
                abstract_text: None,
                academic_status: "unknown".into(),
                access_level: "unknown".into(),
                full_text_available: false,
                rights_uri: None,
                rights_holder: None,
                rights_normalized: "unknown".into(),
                publisher_or_institution: None,
                publication_time: serde_json::json!({"kind":"unknown"}),
                connector_version: "test".into(),
            },
        )
        .await
        .unwrap()
        .0
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn post_anonymous_is_401(pool: sqlx::PgPool) {
        let response = app(state(pool))
            .oneshot(
                Request::post("/api/v1/interactions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"action":"save","target_type":"event","target_id":"00000000-0000-0000-0000-000000000001"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let bytes = axum::body::to_bytes(response.into_body(), 64_000)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["error"]["code"], "unauthenticated");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn delete_foreign_is_404_owned_succeeds(pool: sqlx::PgPool) {
        let person = seed_person(&pool).await;
        let event = seed_event(&pool, person).await;
        let (owner_id, owner_cookie) = session_cookie(&pool, "0x1212121212121212121212121212121212121212").await;
        let (_other_id, other_cookie) =
            session_cookie(&pool, "0x1313131313131313131313131313131313131313").await;
        let created = create_interaction(
            &pool,
            owner_id,
            InteractionAction::Save,
            InteractionTargetType::Event,
            event,
            Some(InteractionVisibility::Private),
        )
        .await
        .unwrap();
        let foreign = app(state(pool.clone()))
            .oneshot(
                Request::delete(format!("/api/v1/interactions/{}", created.row.id))
                    .header("cookie", format!("{}={}", crate::auth::SESSION_COOKIE, other_cookie.value()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(foreign.status(), StatusCode::NOT_FOUND);
        let owned = app(state(pool))
            .oneshot(
                Request::delete(format!("/api/v1/interactions/{}", created.row.id))
                    .header("cookie", format!("{}={}", crate::auth::SESSION_COOKIE, owner_cookie.value()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(owned.status(), StatusCode::NO_CONTENT);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn resolver_rejects_wrong_entity_kind_and_missing(pool: sqlx::PgPool) {
        let person = seed_person(&pool).await;
        let place = seed_place(&pool).await;
        let user = resolve_or_create_evm_user(
            &pool,
            &NormalizedEvmAddress::parse("0x1414141414141414141414141414141414141414").unwrap(),
        )
        .await
        .unwrap();
        let place_as_person = create_interaction(
            &pool,
            user.id,
            InteractionAction::Follow,
            InteractionTargetType::Person,
            place,
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(place_as_person, InteractionError::TargetNotFound);
        let person_as_place = create_interaction(
            &pool,
            user.id,
            InteractionAction::Visited,
            InteractionTargetType::Place,
            person,
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(person_as_place, InteractionError::TargetNotFound);
        let missing = create_interaction(
            &pool,
            user.id,
            InteractionAction::Save,
            InteractionTargetType::Event,
            Uuid::new_v4(),
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(missing, InteractionError::TargetNotFound);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn anonymous_summary_counts_without_mine(pool: sqlx::PgPool) {
        let person = seed_person(&pool).await;
        let claim = seed_claim(&pool, person).await;
        let user = resolve_or_create_evm_user(
            &pool,
            &NormalizedEvmAddress::parse("0x1515151515151515151515151515151515151515").unwrap(),
        )
        .await
        .unwrap();
        create_interaction(
            &pool,
            user.id,
            InteractionAction::Support,
            InteractionTargetType::Claim,
            claim,
            Some(InteractionVisibility::Public),
        )
        .await
        .unwrap();
        create_interaction(
            &pool,
            user.id,
            InteractionAction::Save,
            InteractionTargetType::Claim,
            claim,
            Some(InteractionVisibility::Private),
        )
        .await
        .unwrap();
        let response = app(state(pool))
            .oneshot(
                Request::get(format!(
                    "/api/v1/interactions/summary?target_type=claim&target_ids={claim}"
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 64_000)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["summaries"][0]["counts"]["support"], 1);
        assert_eq!(v["summaries"][0]["counts"]["save"], 0);
        assert_eq!(v["summaries"][0]["comment_count"], 0);
        assert_eq!(v["summaries"][0]["mine"].as_array().unwrap().len(), 0);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn authenticated_summary_includes_private_mine(pool: sqlx::PgPool) {
        let person = seed_person(&pool).await;
        let claim = seed_claim(&pool, person).await;
        let (user_id, cookie) =
            session_cookie(&pool, "0x1717171717171717171717171717171717171717").await;
        create_interaction(
            &pool,
            user_id,
            InteractionAction::Support,
            InteractionTargetType::Claim,
            claim,
            Some(InteractionVisibility::Private),
        )
        .await
        .unwrap();
        let response = app(state(pool))
            .oneshot(
                Request::get(format!(
                    "/api/v1/interactions/summary?target_type=claim&target_ids={claim}"
                ))
                .header("cookie", cookie.encoded().to_string())
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 64_000)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["summaries"][0]["counts"]["support"], 0);
        assert_eq!(v["summaries"][0]["mine"][0], "support");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn typed_create_idempotent_and_source_ok(pool: sqlx::PgPool) {
        let person = seed_person(&pool).await;
        let event = seed_event(&pool, person).await;
        let source = seed_source(&pool).await;
        let (_id, cookie) =
            session_cookie(&pool, "0x1616161616161616161616161616161616161616").await;
        let first = app(state(pool.clone()))
            .oneshot(
                Request::post("/api/v1/interactions")
                    .header("content-type", "application/json")
                    .header("cookie", format!("{}={}", crate::auth::SESSION_COOKIE, cookie.value()))
                    .body(Body::from(format!(
                        r#"{{"action":"save","target_type":"event","target_id":"{event}"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::CREATED);
        let second = app(state(pool.clone()))
            .oneshot(
                Request::post("/api/v1/interactions")
                    .header("content-type", "application/json")
                    .header("cookie", format!("{}={}", crate::auth::SESSION_COOKIE, cookie.value()))
                    .body(Body::from(format!(
                        r#"{{"action":"save","target_type":"event","target_id":"{event}"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(second.status(), StatusCode::OK);
        let src = app(state(pool))
            .oneshot(
                Request::post("/api/v1/interactions")
                    .header("content-type", "application/json")
                    .header("cookie", format!("{}={}", crate::auth::SESSION_COOKIE, cookie.value()))
                    .body(Body::from(format!(
                        r#"{{"action":"useful","target_type":"source","target_id":"{source}"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(src.status(), StatusCode::CREATED);
    }
}
