// crates/talaria-api/src/routes/agora.rs
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::AppState;
use crate::agora_stance::{is_agora_theory, stance_block, StanceBlock, StanceKind};

#[derive(Debug, Deserialize)]
pub struct StanceBody {
    pub stance: String,
    pub min_shares: Option<String>,
}

pub async fn theory_signals(
    State(state): State<AppState>,
    Path(claim_id): Path<Uuid>,
) -> impl IntoResponse {
    match stance_payload(&state, claim_id, None, 1).await {
        Ok((status, body)) => (status, Json(body)).into_response(),
        Err(status) => (status, Json(json!({ "error": "claim_not_found" }))).into_response(),
    }
}

pub async fn theory_simulation(
    State(state): State<AppState>,
    Path(claim_id): Path<Uuid>,
    Json(body): Json<StanceBody>,
) -> impl IntoResponse {
    let Some(stance) = StanceKind::parse(&body.stance) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_stance", "use": ["believe", "dispute"] })),
        )
            .into_response();
    };
    let min_shares = parse_min_shares(body.min_shares.as_deref());
    match stance_payload(&state, claim_id, Some(stance), min_shares).await {
        Ok((status, mut body)) => {
            body["calldata"] = Value::Null;
            body["operation"] = json!("deposit");
            (status, Json(body)).into_response()
        }
        Err(status) => (status, Json(json!({ "error": "claim_not_found" }))).into_response(),
    }
}

pub async fn theory_positions(
    State(state): State<AppState>,
    Path(claim_id): Path<Uuid>,
    Json(body): Json<StanceBody>,
) -> impl IntoResponse {
    theory_simulation(State(state), Path(claim_id), Json(body)).await
}

fn parse_min_shares(raw: Option<&str>) -> u128 {
    raw.and_then(|s| s.trim().parse().ok()).unwrap_or(1)
}

fn block_code(block: StanceBlock) -> &'static str {
    match block {
        StanceBlock::NotTheory => "not_theory",
        StanceBlock::NotOnChain => "not_on_chain",
        StanceBlock::LiveDisabled => "live_disabled",
        StanceBlock::MinSharesForbidden => "min_shares_forbidden",
    }
}

async fn stance_payload(
    state: &AppState,
    claim_id: Uuid,
    stance: Option<StanceKind>,
    min_shares: u128,
) -> Result<(StatusCode, Value), StatusCode> {
    let claim = talaria_store::get_claim(&state.pool, claim_id)
        .await
        .ok()
        .flatten()
        .ok_or(StatusCode::NOT_FOUND)?;
    let pub_row = talaria_store::find_intuition_publication_for_claim(&state.pool, claim_id)
        .await
        .ok()
        .flatten();
    let block = stance_block(
        &claim.claim_kind,
        pub_row.as_ref().map(|row| row.status.as_str()),
        pub_row
            .as_ref()
            .and_then(|row| row.triple_term_id.as_deref()),
        min_shares,
    );
    let Some(block) = block else {
        return Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({ "error": "stance_unhandled" }),
        ));
    };
    let inspecting = stance.is_none();
    let http = match block {
        StanceBlock::NotTheory => StatusCode::BAD_REQUEST,
        StanceBlock::MinSharesForbidden => StatusCode::BAD_REQUEST,
        StanceBlock::LiveDisabled if inspecting => StatusCode::OK,
        StanceBlock::LiveDisabled => StatusCode::SERVICE_UNAVAILABLE,
        StanceBlock::NotOnChain => StatusCode::OK,
    };
    let mut body = json!({
        "status": "blocked",
        "code": block_code(block),
        "actionable": false,
        "claim_id": claim.id,
        "claim_kind": claim.claim_kind,
        "publication_status": pub_row.as_ref().map(|row| row.status.clone()),
        "triple_term_id": pub_row.as_ref().and_then(|row| row.triple_term_id.clone()),
        "calldata": null,
        "reason": match block {
            StanceBlock::NotTheory => "Stance actions are only available on Agora theories.",
            StanceBlock::NotOnChain => "This theory is not on Intuition yet.",
            StanceBlock::LiveDisabled => {
                "intuition-publish --live is disabled until IPFS atom IDs and transaction simulation are fixed."
            }
            StanceBlock::MinSharesForbidden => "minShares = 0 is rejected outside tests.",
        },
    });
    if let Some(stance) = stance {
        body["stance"] = json!(match stance {
            StanceKind::Believe => "believe",
            StanceKind::Dispute => "dispute",
        });
        body["vault"] = json!(stance.vault_role());
    }
    if !is_agora_theory(&claim.claim_kind) {
        body["code"] = json!("not_theory");
    }
    Ok((http, body))
}
