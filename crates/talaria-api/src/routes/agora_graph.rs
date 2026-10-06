// crates/talaria-api/src/routes/agora_graph.rs
use super::auth::require_user;
use super::AppState;
use crate::agora_graph::{
    add_source, create_argument, list_arguments, list_sources, promote_comment, AgoraGraphError,
    CreateArgumentInput,
};
use crate::auth::AuthError;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::CookieJar;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/claims/{claim_id}/sources",
            get(get_sources).post(post_source),
        )
        .route(
            "/api/v1/claims/{claim_id}/arguments",
            get(get_arguments).post(post_argument),
        )
        .route(
            "/api/v1/comments/{comment_id}/promote",
            post(post_promote),
        )
}

fn graph_error(err: AgoraGraphError) -> (StatusCode, Json<Value>) {
    let status = match err {
        AgoraGraphError::Unauthenticated => StatusCode::UNAUTHORIZED,
        AgoraGraphError::ClaimNotFound | AgoraGraphError::CommentNotFound => StatusCode::NOT_FOUND,
        AgoraGraphError::DatabaseError => StatusCode::INTERNAL_SERVER_ERROR,
        AgoraGraphError::SourceAlreadyAttached => StatusCode::OK,
        _ => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(json!({
            "error": { "code": err.code(), "message": err.code() }
        })),
    )
}

fn from_auth(err: AuthError) -> (StatusCode, Json<Value>) {
    super::auth::auth_error(err)
}

#[derive(Deserialize)]
struct SourceBody {
    corpus_document_id: Uuid,
}

#[derive(Deserialize)]
struct ArgumentBody {
    relation: String,
    #[serde(default)]
    statement: String,
    corpus_document_id: Option<Uuid>,
    fragment_id: Option<Uuid>,
    quote: Option<String>,
}

#[derive(Deserialize)]
struct ListQuery {
    limit: Option<i64>,
}

impl ArgumentBody {
    fn into_input(self) -> CreateArgumentInput {
        CreateArgumentInput {
            relation: self.relation,
            statement: self.statement,
            corpus_document_id: self.corpus_document_id,
            fragment_id: self.fragment_id,
            quote: self.quote,
        }
    }
}

async fn get_sources(
    State(state): State<AppState>,
    Path(claim_id): Path<Uuid>,
) -> impl IntoResponse {
    match list_sources(&state.pool, claim_id).await {
        Ok(items) => Json(json!({ "items": items })).into_response(),
        Err(err) => graph_error(err).into_response(),
    }
}

async fn post_source(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(claim_id): Path<Uuid>,
    Json(body): Json<SourceBody>,
) -> impl IntoResponse {
    let user = match require_user(&state.pool, &jar).await {
        Ok(u) => u,
        Err(err) => return from_auth(err).into_response(),
    };
    match add_source(&state.pool, user.id, claim_id, body.corpus_document_id).await {
        Ok((source, created)) => {
            let status = if created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            };
            (status, Json(json!({ "source": source }))).into_response()
        }
        Err(err) => graph_error(err).into_response(),
    }
}

async fn get_arguments(
    State(state): State<AppState>,
    Path(claim_id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    match list_arguments(&state.pool, claim_id, q.limit).await {
        Ok(items) => Json(json!({ "items": items })).into_response(),
        Err(err) => graph_error(err).into_response(),
    }
}

async fn post_argument(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(claim_id): Path<Uuid>,
    Json(body): Json<ArgumentBody>,
) -> impl IntoResponse {
    let user = match require_user(&state.pool, &jar).await {
        Ok(u) => u,
        Err(err) => return from_auth(err).into_response(),
    };
    match create_argument(&state.pool, user.id, claim_id, body.into_input()).await {
        Ok(argument) => (StatusCode::CREATED, Json(json!({ "argument": argument }))).into_response(),
        Err(err) => graph_error(err).into_response(),
    }
}

async fn post_promote(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(comment_id): Path<Uuid>,
    Json(body): Json<ArgumentBody>,
) -> impl IntoResponse {
    let user = match require_user(&state.pool, &jar).await {
        Ok(u) => u,
        Err(err) => return from_auth(err).into_response(),
    };
    match promote_comment(&state.pool, user.id, comment_id, body.into_input()).await {
        Ok(argument) => (StatusCode::CREATED, Json(json!({ "argument": argument }))).into_response(),
        Err(err) => graph_error(err).into_response(),
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
    async fn anonymous_creation_is_401(pool: sqlx::PgPool) {
        let claim = Uuid::new_v4();
        let source = Router::new()
            .merge(router())
            .with_state(AppState {
                pool: pool.clone(),
                offline_only: true,
                config: dummy_config(),
                ingest_jobs: std::sync::Arc::new(tokio::sync::Mutex::new(
                    std::collections::HashMap::new(),
                )),
            })
            .oneshot(
                Request::post(format!("/api/v1/claims/{claim}/sources"))
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"corpus_document_id":"{}"}}"#,
                        Uuid::new_v4()
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(source.status(), StatusCode::UNAUTHORIZED);
        let argument = Router::new()
            .merge(router())
            .with_state(AppState {
                pool,
                offline_only: true,
                config: dummy_config(),
                ingest_jobs: std::sync::Arc::new(tokio::sync::Mutex::new(
                    std::collections::HashMap::new(),
                )),
            })
            .oneshot(
                Request::post(format!("/api/v1/claims/{claim}/arguments"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"relation":"supports","statement":"x","corpus_document_id":null}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(argument.status(), StatusCode::UNAUTHORIZED);
    }
}
