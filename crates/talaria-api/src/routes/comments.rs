// crates/talaria-api/src/routes/comments.rs
use super::auth::{optional_user, require_user};
use super::AppState;
use crate::auth::AuthError;
use crate::comments::{
    add_comment_reaction, create_comment, delete_comment, list_claim_comments, patch_comment,
    remove_comment_reaction, CommentDto, CommentError,
};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use axum_extra::extract::CookieJar;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/claims/{claim_id}/comments",
            get(list_comments).post(post_comment),
        )
        .route(
            "/api/v1/comments/{comment_id}",
            patch(patch_one).delete(delete_one),
        )
        .route(
            "/api/v1/comments/{comment_id}/reactions",
            post(post_reaction),
        )
        .route(
            "/api/v1/comments/{comment_id}/reactions/{reaction_type}",
            delete(delete_reaction),
        )
}

fn comment_error(err: CommentError) -> (StatusCode, Json<Value>) {
    let status = match err {
        CommentError::Unauthenticated => StatusCode::UNAUTHORIZED,
        CommentError::ClaimNotFound | CommentError::CommentNotFound => StatusCode::NOT_FOUND,
        CommentError::DatabaseError => StatusCode::INTERNAL_SERVER_ERROR,
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

#[derive(Deserialize)]
struct ListQuery {
    cursor: Option<String>,
    limit: Option<i64>,
}

#[derive(Deserialize)]
struct PostBody {
    body: String,
    parent_comment_id: Option<Uuid>,
}

#[derive(Deserialize)]
struct PatchBody {
    body: String,
}

#[derive(Deserialize)]
struct ReactionBody {
    reaction_type: String,
}

fn comment_json(dto: &CommentDto) -> Value {
    serde_json::to_value(dto).unwrap_or(json!({}))
}

async fn list_comments(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(claim_id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let viewer = optional_user(&state.pool, &jar).await.map(|u| u.id);
    match list_claim_comments(
        &state.pool,
        claim_id,
        viewer,
        q.cursor.as_deref(),
        q.limit,
    )
    .await
    {
        Ok((items, next_cursor)) => Json(json!({ "items": items, "next_cursor": next_cursor }))
            .into_response(),
        Err(err) => comment_error(err).into_response(),
    }
}

async fn post_comment(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(claim_id): Path<Uuid>,
    Json(body): Json<PostBody>,
) -> impl IntoResponse {
    let user = match require_user(&state.pool, &jar).await {
        Ok(u) => u,
        Err(err) => return from_auth(err).into_response(),
    };
    match create_comment(
        &state.pool,
        user.id,
        claim_id,
        &body.body,
        body.parent_comment_id,
    )
    .await
    {
        Ok(dto) => (StatusCode::CREATED, Json(json!({ "comment": comment_json(&dto) })))
            .into_response(),
        Err(err) => comment_error(err).into_response(),
    }
}

async fn patch_one(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(comment_id): Path<Uuid>,
    Json(body): Json<PatchBody>,
) -> impl IntoResponse {
    let user = match require_user(&state.pool, &jar).await {
        Ok(u) => u,
        Err(err) => return from_auth(err).into_response(),
    };
    match patch_comment(&state.pool, user.id, comment_id, &body.body).await {
        Ok(dto) => Json(json!({ "comment": comment_json(&dto) })).into_response(),
        Err(err) => comment_error(err).into_response(),
    }
}

async fn delete_one(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(comment_id): Path<Uuid>,
) -> impl IntoResponse {
    let user = match require_user(&state.pool, &jar).await {
        Ok(u) => u,
        Err(err) => return from_auth(err).into_response(),
    };
    match delete_comment(&state.pool, user.id, comment_id).await {
        Ok(dto) => Json(json!({ "comment": comment_json(&dto) })).into_response(),
        Err(err) => comment_error(err).into_response(),
    }
}

async fn post_reaction(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(comment_id): Path<Uuid>,
    Json(body): Json<ReactionBody>,
) -> impl IntoResponse {
    let user = match require_user(&state.pool, &jar).await {
        Ok(u) => u,
        Err(err) => return from_auth(err).into_response(),
    };
    match add_comment_reaction(&state.pool, user.id, comment_id, &body.reaction_type).await {
        Ok((dto, created)) => {
            let status = if created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            };
            (status, Json(json!({ "comment": comment_json(&dto) }))).into_response()
        }
        Err(err) => comment_error(err).into_response(),
    }
}

async fn delete_reaction(
    State(state): State<AppState>,
    jar: CookieJar,
    Path((comment_id, reaction_type)): Path<(Uuid, String)>,
) -> impl IntoResponse {
    let user = match require_user(&state.pool, &jar).await {
        Ok(u) => u,
        Err(err) => return from_auth(err).into_response(),
    };
    match remove_comment_reaction(&state.pool, user.id, comment_id, &reaction_type).await {
        Ok(dto) => Json(json!({ "comment": comment_json(&dto) })).into_response(),
        Err(err) => comment_error(err).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    use uuid::Uuid;

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

    #[sqlx::test(migrations = "../../migrations")]
    async fn anonymous_create_is_401(pool: sqlx::PgPool) {
        let app = Router::new().merge(router()).with_state(AppState {
            pool,
            offline_only: true,
            config: dummy_config(),
            ingest_jobs: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
        });
        let response = app
            .oneshot(
                Request::post(format!("/api/v1/claims/{}/comments", Uuid::new_v4()))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"body":"hi"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
