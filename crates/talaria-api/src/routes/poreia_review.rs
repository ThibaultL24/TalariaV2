// crates/talaria-api/src/routes/poreia_review.rs
use super::admin_ingest::require_admin;
use super::AppState;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use serde_json::json;
use talaria_store::{review_poreia_site, valid_reason_code, PoreiaDecision, ReviewOutcome};
use uuid::Uuid;

const OPERATOR_INTENT: &str = "review-site";

#[derive(Debug, Deserialize)]
pub struct ReviewBody {
    decision: String,
    reason_code: String,
    evidence_url: String,
}

fn error(status: StatusCode, code: &'static str) -> Response {
    (status, Json(json!({ "error": code }))).into_response()
}

pub fn valid_evidence_url(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str().is_some()
        && value.len() <= 500
        && !value.contains(' ')
}

pub async fn review_site(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(site_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<ReviewBody>,
) -> Response {
    if let Err(rejection) = require_admin(&state.pool, &jar).await {
        return rejection.into_response();
    }
    let intent = headers
        .get("x-talaria-operator-intent")
        .and_then(|value| value.to_str().ok());
    if intent != Some(OPERATOR_INTENT) {
        return error(StatusCode::BAD_REQUEST, "invalid_operator_intent");
    }
    let Some(decision) = PoreiaDecision::parse(body.decision.trim()) else {
        return error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_decision");
    };
    if !valid_reason_code(body.reason_code.trim()) {
        return error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_reason_code");
    }
    if !valid_evidence_url(body.evidence_url.trim()) {
        return error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_evidence_url");
    }
    let actor = match super::auth::require_user(&state.pool, &jar).await {
        Ok(user) => user.id,
        Err(err) => return super::auth::auth_error(err).into_response(),
    };
    match review_poreia_site(
        &state.pool,
        site_id,
        actor,
        decision,
        body.reason_code.trim(),
        body.evidence_url.trim(),
    )
    .await
    {
        Ok(ReviewOutcome::MissingSite) => error(StatusCode::NOT_FOUND, "site_not_found"),
        Ok(ReviewOutcome::EvidenceRejected) => {
            error(StatusCode::UNPROCESSABLE_ENTITY, "evidence_not_linked")
        }
        Ok(ReviewOutcome::Stored { publication_status }) => Json(json!({
            "site_id": site_id,
            "decision": body.decision.trim(),
            "publication_status": publication_status,
        }))
        .into_response(),
        Err(db_error) => {
            tracing::error!(%db_error, "poreia review failed");
            error(StatusCode::INTERNAL_SERVER_ERROR, "database_error")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_and_intent_fail_closed() {
        assert!(valid_evidence_url(
            "https://www.wikidata.org/wiki/Q59210966"
        ));
        assert!(!valid_evidence_url(
            "http://www.wikidata.org/wiki/Q59210966"
        ));
        assert!(!valid_evidence_url("https://"));
        assert_eq!(OPERATOR_INTENT, "review-site");
    }

    #[test]
    fn unknown_decision_is_not_a_publication() {
        assert!(PoreiaDecision::parse("publish").is_some());
        assert!(PoreiaDecision::parse("auto").is_none());
        assert!(!valid_reason_code("looks_fine"));
        assert!(valid_reason_code("location_verified"));
    }
}
