// crates/talaria-api/src/routes/claim_signals.rs
use super::auth::require_user;
use super::AppState;
use crate::claim_signals::{
    claim_intuition_status, parse_signal_action, sync_claim_signal, HttpIntuitionChain, SignalError,
};
use crate::interactions::InteractionDto;
use axum::extract::{Path, State};
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
        .route("/api/v1/claims/{claim_id}/intuition", get(claim_intuition))
        .route(
            "/api/v1/claims/{claim_id}/signals/sync",
            post(sync_signal),
        )
}

fn signal_error(err: SignalError) -> (StatusCode, Json<Value>) {
    let status = match err {
        SignalError::Unauthenticated => StatusCode::UNAUTHORIZED,
        SignalError::ClaimNotFound => StatusCode::NOT_FOUND,
        SignalError::DuplicateTx => StatusCode::CONFLICT,
        SignalError::DatabaseError | SignalError::RpcError => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(json!({
            "error": { "code": err.code(), "message": err.message() }
        })),
    )
}

async fn claim_intuition(
    State(state): State<AppState>,
    Path(claim_id): Path<Uuid>,
) -> impl IntoResponse {
    match claim_intuition_status(&state.pool, claim_id).await {
        Ok(body) => Json(body).into_response(),
        Err(err) => signal_error(err).into_response(),
    }
}

#[derive(Deserialize)]
struct SyncBody {
    action: String,
    tx_hash: String,
}

async fn sync_signal(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(claim_id): Path<Uuid>,
    Json(body): Json<SyncBody>,
) -> impl IntoResponse {
    let user = match require_user(&state.pool, &jar).await {
        Ok(u) => u,
        Err(_) => return signal_error(SignalError::Unauthenticated).into_response(),
    };
    let action = match parse_signal_action(&body.action) {
        Ok(a) => a,
        Err(err) => return signal_error(err).into_response(),
    };
    let chain = HttpIntuitionChain::for_chain(crate::claim_signals::configured_chain_id());
    match sync_claim_signal(&state.pool, &chain, &user, claim_id, action, &body.tx_hash).await {
        Ok((interaction, created)) => {
            let status = if created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            };
            (
                status,
                Json(json!({
                    "interaction": dto_json(&interaction),
                    "synced": true
                })),
            )
                .into_response()
        }
        Err(err) => signal_error(err).into_response(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
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

    #[sqlx::test(migrations = "../../migrations")]
    async fn sync_anonymous_is_401(pool: sqlx::PgPool) {
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
                Request::post(format!(
                    "/api/v1/claims/{}/signals/sync",
                    Uuid::new_v4()
                ))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"action":"support","tx_hash":"0xcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"}"#))
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
