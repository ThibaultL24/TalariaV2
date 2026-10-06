// crates/talaria-api/src/routes/auth.rs
use super::AppState;
use crate::auth::{
    issue_wallet_challenge, verify_wallet_challenge, AuthError, SESSION_COOKIE, SESSION_TTL_DAYS,
};
use axum::extract::{Json, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Deserialize;
use serde_json::{json, Value};
use talaria_store::{get_active_session_user, hash_session_token, revoke_session};
use time::Duration as TimeDuration;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct ChallengeBody {
    address: String,
    chain_id: Option<u64>,
}

#[derive(Deserialize)]
pub struct VerifyBody {
    challenge_id: Uuid,
    message: String,
    signature: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/auth/wallet/challenge", post(challenge))
        .route("/api/v1/auth/wallet/verify", post(verify))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/me", get(me))
}

pub(crate) fn auth_error(err: AuthError) -> (StatusCode, Json<Value>) {
    let status = match err {
        AuthError::Unauthenticated => StatusCode::UNAUTHORIZED,
        AuthError::UnknownChallenge => StatusCode::NOT_FOUND,
        AuthError::Conflict => StatusCode::CONFLICT,
        AuthError::DatabaseError => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(json!({
            "error": { "code": err.code(), "message": err.message() }
        })),
    )
}

fn cookie_secure() -> bool {
    matches!(
        std::env::var("TALARIA_COOKIE_SECURE").ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "on")
    )
}

fn session_cookie(token: String) -> Cookie<'static> {
    let mut cookie = Cookie::new(SESSION_COOKIE, token);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_path("/");
    cookie.set_secure(cookie_secure());
    cookie.set_max_age(TimeDuration::days(SESSION_TTL_DAYS));
    cookie
}

fn clear_session_cookie() -> Cookie<'static> {
    let mut cookie = Cookie::from(SESSION_COOKIE);
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_max_age(TimeDuration::ZERO);
    cookie
}

fn siwe_domain(headers: &HeaderMap) -> String {
    if let Ok(domain) = std::env::var("TALARIA_AUTH_DOMAIN") {
        let trimmed = domain.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost")
        .to_string()
}

fn public_origin(headers: &HeaderMap) -> String {
    if let Ok(origin) = std::env::var("TALARIA_PUBLIC_ORIGIN") {
        let trimmed = origin.trim().trim_end_matches('/');
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    format!("http://{}", siwe_domain(headers))
}

async fn challenge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ChallengeBody>,
) -> impl IntoResponse {
    let chain_id = body.chain_id.unwrap_or(13579);
    match issue_wallet_challenge(
        &state.pool,
        &body.address,
        chain_id,
        &siwe_domain(&headers),
        &public_origin(&headers),
    )
    .await
    {
        Ok(issued) => (
            StatusCode::OK,
            Json(json!({
                "challenge_id": issued.id,
                "message": issued.message
            })),
        ),
        Err(err) => auth_error(err),
    }
}

async fn verify(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<VerifyBody>,
) -> impl IntoResponse {
    match verify_wallet_challenge(
        &state.pool,
        body.challenge_id,
        &body.message,
        &body.signature,
    )
    .await
    {
        Ok(session) => {
            let jar = jar.add(session_cookie(session.session_token));
            (
                jar,
                Json(json!({
                    "user": {
                        "id": session.user.id,
                        "wallet_address": session.wallet_address
                    }
                })),
            )
                .into_response()
        }
        Err(err) => auth_error(err).into_response(),
    }
}

async fn logout(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    if let Some(token) = jar.get(SESSION_COOKIE).map(|c| c.value().to_string()) {
        let _ = revoke_session(&state.pool, &hash_session_token(&token), chrono::Utc::now()).await;
    }
    let jar = jar.add(clear_session_cookie());
    (jar, Json(json!({ "ok": true }))).into_response()
}

pub(crate) async fn optional_user(
    pool: &sqlx::PgPool,
    jar: &CookieJar,
) -> Option<talaria_store::UserRow> {
    let token = jar.get(SESSION_COOKIE)?.value();
    talaria_store::get_active_session_user(
        pool,
        &talaria_store::hash_session_token(token),
        chrono::Utc::now(),
    )
    .await
    .ok()
    .flatten()
}

pub(crate) async fn require_user(
    pool: &sqlx::PgPool,
    jar: &CookieJar,
) -> Result<talaria_store::UserRow, AuthError> {
    optional_user(pool, jar)
        .await
        .ok_or(AuthError::Unauthenticated)
}

async fn me(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let Some(token) = jar.get(SESSION_COOKIE).map(|c| c.value().to_string()) else {
        return Json(json!({ "authenticated": false }));
    };
    match get_active_session_user(
        &state.pool,
        &hash_session_token(&token),
        chrono::Utc::now(),
    )
    .await
    {
        Ok(Some(user)) => {
            let wallet = talaria_store::evm_address_for_user(&state.pool, user.id)
                .await
                .ok()
                .flatten();
            Json(json!({
                "authenticated": true,
                "user": {
                    "id": user.id,
                    "wallet_address": wallet
                }
            }))
        }
        _ => Json(json!({ "authenticated": false })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{build_siwe_message, new_nonce, parse_signature, verify_wallet_challenge};
    use alloy_signer::SignerSync;
    use alloy_signer_local::PrivateKeySigner;
    use talaria_store::{
        consume_wallet_challenge, insert_wallet_challenge, resolve_or_create_evm_user,
        NormalizedEvmAddress,
    };
    use time::OffsetDateTime;

    fn sign(signer: &PrivateKeySigner, message: &str) -> String {
        let sig = signer.sign_message_sync(message.as_bytes()).unwrap();
        format!("0x{}", hex::encode(sig.as_bytes()))
    }

    #[test]
    fn parse_signature_rejects_short_hex() {
        assert!(parse_signature("0x11").is_err());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn wallet_auth_handshake_and_security(pool: sqlx::PgPool) {
        let signer = PrivateKeySigner::random();
        let address = format!("{:#x}", signer.address());
        let checksum = signer.address().to_checksum(None);
        let now = OffsetDateTime::now_utc();
        let exp = now + time::Duration::minutes(10);
        let nonce = new_nonce();
        let parsed = NormalizedEvmAddress::parse(&checksum).unwrap();
        assert_eq!(parsed.as_stored(), address);

        let message = build_siwe_message(
            &parsed,
            13579,
            "localhost:5173",
            "http://localhost:5173",
            &nonce,
            now,
            exp,
        )
        .unwrap();
        let challenge_id = insert_wallet_challenge(
            &pool,
            &parsed,
            &nonce,
            &message,
            chrono::Utc::now() + chrono::Duration::minutes(10),
        )
        .await
        .unwrap();

        let other = PrivateKeySigner::random();
        let other_sig = sign(&other, &message);
        let wrong_wallet = verify_wallet_challenge(&pool, challenge_id, &message, &other_sig).await;
        assert_eq!(wrong_wallet.unwrap_err(), crate::auth::AuthError::InvalidSignature);

        let mutated = message.replace("Sign in to Talaria", "Sign in to Other");
        let ok_sig = sign(&signer, &message);
        let mismatch = verify_wallet_challenge(&pool, challenge_id, &mutated, &ok_sig).await;
        assert_eq!(mismatch.unwrap_err(), crate::auth::AuthError::MessageMismatch);

        let first = verify_wallet_challenge(&pool, challenge_id, &message, &ok_sig)
            .await
            .unwrap();
        let replay = verify_wallet_challenge(&pool, challenge_id, &message, &ok_sig).await;
        assert_eq!(replay.unwrap_err(), crate::auth::AuthError::ChallengeReplay);

        let checksum_again = NormalizedEvmAddress::parse(&checksum).unwrap();
        let again = resolve_or_create_evm_user(&pool, &checksum_again)
            .await
            .unwrap();
        assert_eq!(again.id, first.user.id);

        let other_user = resolve_or_create_evm_user(
            &pool,
            &NormalizedEvmAddress::parse(&format!("{:#x}", other.address())).unwrap(),
        )
        .await
        .unwrap();
        assert_ne!(other_user.id, first.user.id);

        let unknown = verify_wallet_challenge(&pool, Uuid::new_v4(), &message, &ok_sig).await;
        assert_eq!(unknown.unwrap_err(), crate::auth::AuthError::UnknownChallenge);

        let expired_id = insert_wallet_challenge(
            &pool,
            &parsed,
            &new_nonce(),
            &message,
            chrono::Utc::now() - chrono::Duration::minutes(1),
        )
        .await
        .unwrap();
        let expired = verify_wallet_challenge(&pool, expired_id, &message, &ok_sig).await;
        assert_eq!(expired.unwrap_err(), crate::auth::AuthError::ExpiredChallenge);

        let consumed_early = insert_wallet_challenge(
            &pool,
            &parsed,
            &new_nonce(),
            &message,
            chrono::Utc::now() + chrono::Duration::minutes(10),
        )
        .await
        .unwrap();
        consume_wallet_challenge(&pool, consumed_early, chrono::Utc::now())
            .await
            .unwrap();
        let replay2 = verify_wallet_challenge(&pool, consumed_early, &message, &ok_sig).await;
        assert_eq!(replay2.unwrap_err(), crate::auth::AuthError::ChallengeReplay);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn duplicate_session_maps_to_conflict_not_invalid_address(pool: sqlx::PgPool) {
        let parsed = NormalizedEvmAddress::parse(&format!("{:#x}", PrivateKeySigner::random().address()))
            .unwrap();
        let user = resolve_or_create_evm_user(&pool, &parsed).await.unwrap();
        let hash = "bb".repeat(32);
        let exp = chrono::Utc::now() + chrono::Duration::hours(1);
        talaria_store::insert_session(&pool, user.id, &hash, exp)
            .await
            .unwrap();
        let err = talaria_store::insert_session(&pool, user.id, &hash, exp)
            .await
            .unwrap_err();
        let mapped = crate::auth::map_db_error(err);
        assert_eq!(mapped, crate::auth::AuthError::Conflict);
        assert_eq!(mapped.code(), "conflict");
        assert_ne!(mapped.code(), "invalid_address");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn session_me_and_logout(pool: sqlx::PgPool) {
        let signer = PrivateKeySigner::random();
        let parsed = NormalizedEvmAddress::parse(&format!("{:#x}", signer.address())).unwrap();
        let now = OffsetDateTime::now_utc();
        let nonce = new_nonce();
        let message = build_siwe_message(
            &parsed,
            1155,
            "localhost",
            "http://localhost",
            &nonce,
            now,
            now + time::Duration::minutes(10),
        )
        .unwrap();
        let challenge_id = insert_wallet_challenge(
            &pool,
            &parsed,
            &nonce,
            &message,
            chrono::Utc::now() + chrono::Duration::minutes(10),
        )
        .await
        .unwrap();
        let sig = sign(&signer, &message);
        let session = verify_wallet_challenge(&pool, challenge_id, &message, &sig)
            .await
            .unwrap();
        let token_hash = talaria_store::hash_session_token(&session.session_token);
        let user = get_active_session_user(&pool, &token_hash, chrono::Utc::now())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user.id, session.user.id);
        assert!(revoke_session(&pool, &token_hash, chrono::Utc::now())
            .await
            .unwrap());
        assert!(get_active_session_user(&pool, &token_hash, chrono::Utc::now())
            .await
            .unwrap()
            .is_none());
    }
}
