// crates/talaria-api/src/auth.rs
//! Talaria off-chain wallet authentication (SIWE / EIP-4361).
//! Session is bound to the recovered address, not to Intuition chainId.

use chrono::{Duration, Utc};
use rand::Rng;
use siwe::{Message, TimeStamp, Version};
use talaria_store::{
    consume_wallet_challenge, evm_address_for_user, get_wallet_challenge, hash_session_token,
    insert_session, insert_wallet_challenge, is_unique_violation, resolve_or_create_evm_user,
    NormalizedEvmAddress, UserRow,
};
use time::OffsetDateTime;
use uuid::Uuid;

pub const SESSION_COOKIE: &str = "talaria_session";
pub const CHALLENGE_TTL_MINUTES: i64 = 10;
pub const SESSION_TTL_DAYS: i64 = 14;
pub const SIWE_STATEMENT: &str =
    "Sign in to Talaria. This signature creates a session, spends no gas, and does not submit an Intuition transaction.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    InvalidAddress,
    InvalidChainId,
    UnknownChallenge,
    ExpiredChallenge,
    ChallengeReplay,
    MessageMismatch,
    InvalidSignature,
    Unauthenticated,
    Conflict,
    DatabaseError,
}

impl AuthError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidAddress => "invalid_address",
            Self::InvalidChainId => "invalid_chain_id",
            Self::UnknownChallenge => "unknown_challenge",
            Self::ExpiredChallenge => "expired_challenge",
            Self::ChallengeReplay => "challenge_replay",
            Self::MessageMismatch => "message_mismatch",
            Self::InvalidSignature => "invalid_signature",
            Self::Unauthenticated => "unauthenticated",
            Self::Conflict => "conflict",
            Self::DatabaseError => "database_error",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::InvalidAddress => "Wallet address is not a valid EVM address",
            Self::InvalidChainId => "SIWE chainId must be a positive integer",
            Self::UnknownChallenge => "Unknown authentication challenge",
            Self::ExpiredChallenge => "Authentication challenge expired",
            Self::ChallengeReplay => "Authentication challenge already used",
            Self::MessageMismatch => "Signed message does not match the issued challenge",
            Self::InvalidSignature => "Signature does not match the challenged wallet",
            Self::Unauthenticated => "Not authenticated",
            Self::Conflict => "The request conflicts with existing state",
            Self::DatabaseError => "Internal database error",
        }
    }
}

/// Map store/sqlx failures. Never leak SQL text. Never collapse to `invalid_address`.
pub fn map_db_error(err: anyhow::Error) -> AuthError {
    tracing::error!(error = %err, "database error");
    if is_unique_violation(&err) {
        AuthError::Conflict
    } else {
        AuthError::DatabaseError
    }
}

pub fn new_nonce() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn new_session_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn parse_signature(raw: &str) -> Result<[u8; 65], AuthError> {
    let hex = raw.trim().strip_prefix("0x").unwrap_or(raw.trim());
    let bytes = hex::decode(hex).map_err(|_| AuthError::InvalidSignature)?;
    bytes.try_into().map_err(|_| AuthError::InvalidSignature)
}

pub fn build_siwe_message(
    address: &NormalizedEvmAddress,
    chain_id: u64,
    domain: &str,
    uri: &str,
    nonce: &str,
    issued_at: OffsetDateTime,
    expires_at: OffsetDateTime,
) -> Result<String, AuthError> {
    if chain_id == 0 {
        return Err(AuthError::InvalidChainId);
    }
    let msg = Message {
        domain: domain.parse().map_err(|_| AuthError::InvalidAddress)?,
        address: address.alloy().into_array(),
        statement: Some(SIWE_STATEMENT.to_string()),
        uri: uri.parse().map_err(|_| AuthError::InvalidAddress)?,
        version: Version::V1,
        chain_id,
        nonce: nonce.to_string(),
        issued_at: TimeStamp::from(issued_at),
        expiration_time: Some(TimeStamp::from(expires_at)),
        not_before: None,
        request_id: None,
        resources: vec![],
    };
    Ok(msg.to_string())
}

#[derive(Debug)]
pub struct IssuedChallenge {
    pub id: Uuid,
    pub message: String,
}

pub async fn issue_wallet_challenge(
    pool: &sqlx::PgPool,
    address_raw: &str,
    chain_id: u64,
    domain: &str,
    uri: &str,
) -> Result<IssuedChallenge, AuthError> {
    let address = NormalizedEvmAddress::parse(address_raw).map_err(|_| AuthError::InvalidAddress)?;
    if chain_id == 0 {
        return Err(AuthError::InvalidChainId);
    }
    let nonce = new_nonce();
    let now = OffsetDateTime::now_utc();
    let exp = now + time::Duration::minutes(CHALLENGE_TTL_MINUTES);
    let message = build_siwe_message(&address, chain_id, domain, uri, &nonce, now, exp)?;
    let expires_at = Utc::now() + Duration::minutes(CHALLENGE_TTL_MINUTES);
    let id = insert_wallet_challenge(pool, &address, &nonce, &message, expires_at)
        .await
        .map_err(map_db_error)?;
    Ok(IssuedChallenge { id, message })
}

#[derive(Debug)]
pub struct VerifiedSession {
    pub user: UserRow,
    pub wallet_address: String,
    pub session_token: String,
}

pub async fn verify_wallet_challenge(
    pool: &sqlx::PgPool,
    challenge_id: Uuid,
    message: &str,
    signature: &str,
) -> Result<VerifiedSession, AuthError> {
    let row = get_wallet_challenge(pool, challenge_id)
        .await
        .map_err(map_db_error)?
        .ok_or(AuthError::UnknownChallenge)?;
    let now = Utc::now();
    if row.consumed_at.is_some() {
        return Err(AuthError::ChallengeReplay);
    }
    if row.expires_at <= now {
        return Err(AuthError::ExpiredChallenge);
    }
    if row.message != message {
        return Err(AuthError::MessageMismatch);
    }
    let parsed: Message = message.parse().map_err(|_| AuthError::MessageMismatch)?;
    let address = NormalizedEvmAddress::parse(&row.address).map_err(|_| AuthError::InvalidAddress)?;
    if parsed.address != address.alloy().into_array() {
        return Err(AuthError::MessageMismatch);
    }
    let sig = parse_signature(signature)?;
    parsed
        .verify_eip191(&sig)
        .map_err(|_| AuthError::InvalidSignature)?;
    let consumed = consume_wallet_challenge(pool, challenge_id, now)
        .await
        .map_err(map_db_error)?;
    if !consumed {
        return Err(AuthError::ChallengeReplay);
    }
    let user = resolve_or_create_evm_user(pool, &address)
        .await
        .map_err(map_db_error)?;
    let session_token = new_session_token();
    let expires_at = now + Duration::days(SESSION_TTL_DAYS);
    insert_session(pool, user.id, &hash_session_token(&session_token), expires_at)
        .await
        .map_err(map_db_error)?;
    let wallet_address = evm_address_for_user(pool, user.id)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| address.as_stored());
    Ok(VerifiedSession {
        user,
        wallet_address,
        session_token,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_address_before_siwe() {
        assert!(NormalizedEvmAddress::parse("0x123").is_err());
    }

    #[test]
    fn signature_must_be_65_bytes() {
        assert!(parse_signature("0xabcd").is_err());
    }

    #[test]
    fn generic_sql_is_database_error_not_invalid_address() {
        let err = map_db_error(anyhow::anyhow!("connection refused"));
        assert_eq!(err.code(), "database_error");
        assert_ne!(err.code(), "invalid_address");
    }

    #[test]
    fn auth_error_codes_are_distinct() {
        assert_eq!(AuthError::InvalidAddress.code(), "invalid_address");
        assert_eq!(AuthError::Unauthenticated.code(), "unauthenticated");
        assert_eq!(AuthError::Conflict.code(), "conflict");
        assert_eq!(AuthError::DatabaseError.code(), "database_error");
        assert_eq!(AuthError::ExpiredChallenge.code(), "expired_challenge");
        assert_eq!(AuthError::InvalidSignature.code(), "invalid_signature");
    }
}
