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
use crate::agora_stance::{
    is_stance_claim, modeled_target_block, stance_block, StanceBlock, StanceKind, StanceTargetKind,
};

#[derive(Debug, Deserialize)]
pub struct StanceBody {
    pub stance: String,
    pub min_shares: Option<String>,
}

pub async fn theory_signals(
    State(state): State<AppState>,
    Path(claim_id): Path<Uuid>,
) -> impl IntoResponse {
    match claim_stance_payload(&state, claim_id, None, 1).await {
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
    match claim_stance_payload(&state, claim_id, Some(stance), min_shares).await {
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

/// Generic Intuition stance for person | event | claim | source (doctrine §04).
pub async fn target_signals(
    State(state): State<AppState>,
    Path((kind, target_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match target_stance_payload(&state, &kind, &target_id, None, 1).await {
        Ok((status, body)) => (status, Json(body)).into_response(),
        Err((status, err)) => (status, Json(json!({ "error": err }))).into_response(),
    }
}

pub async fn target_simulation(
    State(state): State<AppState>,
    Path((kind, target_id)): Path<(String, String)>,
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
    match target_stance_payload(&state, &kind, &target_id, Some(stance), min_shares).await {
        Ok((status, mut body)) => {
            body["calldata"] = Value::Null;
            body["operation"] = json!("deposit");
            (status, Json(body)).into_response()
        }
        Err((status, err)) => (status, Json(json!({ "error": err }))).into_response(),
    }
}

fn parse_min_shares(raw: Option<&str>) -> u128 {
    raw.and_then(|s| s.trim().parse().ok()).unwrap_or(1)
}

fn block_code(block: StanceBlock) -> &'static str {
    match block {
        StanceBlock::NotTheory => "not_eligible",
        StanceBlock::NotOnChain => "not_on_chain",
        StanceBlock::ModeledPending => "modeled",
        StanceBlock::LiveDisabled => "live_disabled",
        StanceBlock::Ready => "ready",
        StanceBlock::MinSharesForbidden => "min_shares_forbidden",
    }
}

fn reason_for(block: StanceBlock, target: &str) -> String {
    match block {
        StanceBlock::NotTheory => {
            "Stance actions are only available on Intuition-eligible targets (person, event, theory/controversy, interpretation, source)."
                .into()
        }
        StanceBlock::NotOnChain => format!("This {target} is not on Intuition yet."),
        StanceBlock::ModeledPending => {
            if crate::intuition::live_publish_allowed() {
                format!(
                    "Modeled Intuition signal on this {target} — ready for testnet publish when the operator enables live settle."
                )
            } else {
                format!(
                    "Modeled Intuition signal on this {target}. Believe / Dispute records community trust without rewriting evidence."
                )
            }
        }
        StanceBlock::LiveDisabled => {
            "Set INTUITION_ALLOW_LIVE=1 and INTUITION_PRIVATE_KEY to enable testnet deposit previews.".into()
        }
        StanceBlock::Ready => {
            "Published on Intuition testnet — Believe deposits the vote triple; Dispute deposits the counter-triple (preview first)."
                .into()
        }
        StanceBlock::MinSharesForbidden => "minShares = 0 is rejected outside tests.".into(),
    }
}

async fn claim_stance_payload(
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
        crate::intuition::live_publish_allowed(),
    )
    .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut response = stance_response(
        block,
        stance,
        "claim",
        &claim.id.to_string(),
        Some(claim.claim_kind.as_str()),
        pub_row.as_ref().map(|row| row.status.as_str()),
        pub_row
            .as_ref()
            .and_then(|row| row.triple_term_id.clone()),
        pub_row
            .as_ref()
            .and_then(|row| row.payload_json.clone()),
    );
    if matches!(block, StanceBlock::Ready) {
        if let Some(stance) = stance {
            if let Some(triple_id) = pub_row
                .as_ref()
                .and_then(|row| row.triple_term_id.as_deref())
            {
                if let Ok(preview) =
                    crate::intuition::run_stance_preview(triple_id, stance).await
                {
                    let previewed = preview.get("status").and_then(|s| s.as_str())
                        == Some("previewed");
                    response.1["preview"] = preview;
                    response.1["actionable"] = json!(true);
                    response.1["status"] = json!("ok");
                    if previewed {
                        response.0 = StatusCode::OK;
                    }
                }
            }
        } else {
            response.1["actionable"] = json!(true);
            response.1["status"] = json!("ok");
        }
    }
    Ok(response)
}

async fn target_stance_payload(
    state: &AppState,
    kind_raw: &str,
    target_id: &str,
    stance: Option<StanceKind>,
    min_shares: u128,
) -> Result<(StatusCode, Value), (StatusCode, &'static str)> {
    let Some(kind) = StanceTargetKind::parse(kind_raw) else {
        return Err((
            StatusCode::BAD_REQUEST,
            "invalid_target_kind",
        ));
    };
    match kind {
        StanceTargetKind::Claim => {
            let claim_id = Uuid::parse_str(target_id)
                .map_err(|_| (StatusCode::BAD_REQUEST, "invalid_target_id"))?;
            claim_stance_payload(state, claim_id, stance, min_shares)
                .await
                .map_err(|status| {
                    (
                        status,
                        if status == StatusCode::NOT_FOUND {
                            "claim_not_found"
                        } else {
                            "stance_error"
                        },
                    )
                })
        }
        StanceTargetKind::Person => {
            let entity_id = Uuid::parse_str(target_id)
                .map_err(|_| (StatusCode::BAD_REQUEST, "invalid_target_id"))?;
            let entity = talaria_store::get_entity(&state.pool, entity_id)
                .await
                .ok()
                .flatten()
                .ok_or((StatusCode::NOT_FOUND, "person_not_found"))?;
            let block = modeled_target_block(min_shares);
            Ok(stance_response(
                block,
                stance,
                "person",
                &entity.id.to_string(),
                None,
                Some("modeled"),
                None,
                None,
            ))
        }
        StanceTargetKind::Event => {
            let event_id = Uuid::parse_str(target_id)
                .map_err(|_| (StatusCode::BAD_REQUEST, "invalid_target_id"))?;
            let event = talaria_store::get_canonical_event(&state.pool, event_id)
                .await
                .ok()
                .flatten()
                .ok_or((StatusCode::NOT_FOUND, "event_not_found"))?;
            let block = modeled_target_block(min_shares);
            Ok(stance_response(
                block,
                stance,
                "event",
                &event.id.to_string(),
                None,
                Some("modeled"),
                None,
                None,
            ))
        }
        StanceTargetKind::Source => {
            // Source ids are opaque fingerprints (url hash / document uuid / evidence id).
            if target_id.trim().is_empty() || target_id.len() > 200 {
                return Err((StatusCode::BAD_REQUEST, "invalid_target_id"));
            }
            let block = modeled_target_block(min_shares);
            Ok(stance_response(
                block,
                stance,
                "source",
                target_id.trim(),
                None,
                Some("modeled"),
                None,
                None,
            ))
        }
    }
}

fn stance_response(
    block: StanceBlock,
    stance: Option<StanceKind>,
    target_kind: &str,
    target_id: &str,
    claim_kind: Option<&str>,
    publication_status: Option<&str>,
    triple_term_id: Option<String>,
    payload_json: Option<Value>,
) -> (StatusCode, Value) {
    let inspecting = stance.is_none();
    let http = match block {
        StanceBlock::NotTheory => StatusCode::BAD_REQUEST,
        StanceBlock::MinSharesForbidden => StatusCode::BAD_REQUEST,
        StanceBlock::LiveDisabled if inspecting => StatusCode::OK,
        StanceBlock::LiveDisabled => StatusCode::SERVICE_UNAVAILABLE,
        StanceBlock::Ready | StanceBlock::NotOnChain | StanceBlock::ModeledPending => {
            StatusCode::OK
        }
    };
    let mut body = json!({
        "status": if matches!(block, StanceBlock::Ready) { "ok" } else { "blocked" },
        "code": block_code(block),
        "actionable": matches!(block, StanceBlock::Ready),
        "target_kind": target_kind,
        "target_id": target_id,
        "claim_id": if target_kind == "claim" { json!(target_id) } else { Value::Null },
        "claim_kind": claim_kind,
        "publication_status": publication_status,
        "triple_term_id": triple_term_id,
        "calldata": null,
        "intuition_network": "testnet",
        "intuition_live_allowed": crate::intuition::live_publish_allowed(),
        "reason": reason_for(block, target_kind),
    });
    if let Some(stance) = stance {
        body["stance"] = json!(match stance {
            StanceKind::Believe => "believe",
            StanceKind::Dispute => "dispute",
        });
        body["vault"] = json!(stance.vault_role());
    }
    if let Some(graph) = payload_json.as_ref().and_then(|p| p.get("graph")) {
        if let Some(vote) = graph.get("voteTripleId") {
            body["vote_triple_id"] = vote.clone();
        }
        if let Some(cat) = graph.get("category") {
            body["category"] = cat.clone();
        }
    }
    if claim_kind.is_some_and(|k| !is_stance_claim(k)) {
        body["code"] = json!("not_eligible");
    }
    (http, body)
}
