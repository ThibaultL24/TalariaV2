// crates/talaria-store/src/users.rs
//! Application users and wallet identities (off-chain Talaria auth).
//!
//! Invariant: `user_identities.external_identifier` for `evm_wallet` is a
//! lowercase `0x`-prefixed 20-byte hex address. Display checksums are never stored.

use alloy_primitives::Address;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::str::FromStr;
use uuid::Uuid;

pub const EVM_WALLET_PROVIDER: &str = "evm_wallet";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizedEvmAddress(Address);

impl NormalizedEvmAddress {
    /// Parse via `alloy_primitives::Address` (EIP-55 checksum when mixed-case).
    /// Stored form is always lowercase `0x` + 40 hex chars.
    pub fn parse(raw: &str) -> Result<Self, AddressError> {
        let addr = Address::from_str(raw.trim()).map_err(|_| AddressError::Invalid)?;
        Ok(Self(addr))
    }

    pub fn as_stored(&self) -> String {
        format!("{:#x}", self.0)
    }

    pub fn alloy(&self) -> Address {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressError {
    Invalid,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuthChallengeRow {
    pub id: Uuid,
    pub address: String,
    pub nonce: String,
    pub message: String,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

pub fn hash_session_token(raw: &str) -> String {
    hex::encode(Sha256::digest(raw.as_bytes()))
}

pub fn postgres_error_code(err: &anyhow::Error) -> Option<String> {
    for cause in err.chain() {
        if let Some(sqlx::Error::Database(db)) = cause.downcast_ref::<sqlx::Error>() {
            return db.code().map(|c| c.into_owned());
        }
    }
    None
}

pub fn is_unique_violation(err: &anyhow::Error) -> bool {
    postgres_error_code(err).as_deref() == Some("23505")
}

pub fn is_check_violation(err: &anyhow::Error) -> bool {
    postgres_error_code(err).as_deref() == Some("23514")
}

pub async fn insert_wallet_challenge(
    pool: &PgPool,
    address: &NormalizedEvmAddress,
    nonce: &str,
    message: &str,
    expires_at: DateTime<Utc>,
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO wallet_auth_challenges (id, address, nonce, message, expires_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(id)
    .bind(address.as_stored())
    .bind(nonce)
    .bind(message)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn get_wallet_challenge(
    pool: &PgPool,
    challenge_id: Uuid,
) -> anyhow::Result<Option<AuthChallengeRow>> {
    let row = sqlx::query_as::<_, AuthChallengeRow>(
        r#"
        SELECT id, address, nonce, message, expires_at, consumed_at
        FROM wallet_auth_challenges
        WHERE id = $1
        "#,
    )
    .bind(challenge_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Marks the challenge consumed. Returns true if this call won the race.
pub async fn consume_wallet_challenge(
    pool: &PgPool,
    challenge_id: Uuid,
    now: DateTime<Utc>,
) -> anyhow::Result<bool> {
    let n = sqlx::query(
        r#"
        UPDATE wallet_auth_challenges
        SET consumed_at = $2
        WHERE id = $1
          AND consumed_at IS NULL
          AND expires_at > $2
        "#,
    )
    .bind(challenge_id)
    .bind(now)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(n == 1)
}

pub async fn lookup_user_by_evm(
    pool: &PgPool,
    address: &NormalizedEvmAddress,
) -> anyhow::Result<Option<UserRow>> {
    let row = sqlx::query_as::<_, UserRow>(
        r#"
        SELECT u.id, u.display_name, u.created_at
        FROM user_identities i
        JOIN users u ON u.id = i.user_id
        WHERE i.provider = $1 AND i.external_identifier = $2
        "#,
    )
    .bind(EVM_WALLET_PROVIDER)
    .bind(address.as_stored())
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Idempotent: unique (provider, address) wins under concurrency.
pub async fn resolve_or_create_evm_user(
    pool: &PgPool,
    address: &NormalizedEvmAddress,
) -> anyhow::Result<UserRow> {
    if let Some(existing) = lookup_user_by_evm(pool, address).await? {
        return Ok(existing);
    }
    let stored = address.as_stored();
    let mut tx = pool.begin().await?;
    let user_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO users (id) VALUES ($1)
        "#,
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    let inserted = sqlx::query(
        r#"
        INSERT INTO user_identities (user_id, provider, external_identifier)
        VALUES ($1, $2, $3)
        ON CONFLICT (provider, external_identifier) DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(EVM_WALLET_PROVIDER)
    .bind(&stored)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if inserted == 0 {
        tx.rollback().await?;
        sqlx::query("DELETE FROM users WHERE id = $1 AND NOT EXISTS (SELECT 1 FROM user_identities WHERE user_id = $1)")
            .bind(user_id)
            .execute(pool)
            .await?;
        return lookup_user_by_evm(pool, address)
            .await?
            .ok_or_else(|| anyhow::anyhow!("identity conflict without existing user"));
    }
    tx.commit().await?;
    lookup_user_by_evm(pool, address)
        .await?
        .ok_or_else(|| anyhow::anyhow!("user missing after insert"))
}

pub async fn insert_session(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    expires_at: DateTime<Utc>,
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO user_sessions (id, user_id, token_hash, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn get_active_session_user(
    pool: &PgPool,
    token_hash: &str,
    now: DateTime<Utc>,
) -> anyhow::Result<Option<UserRow>> {
    let row = sqlx::query_as::<_, UserRow>(
        r#"
        SELECT u.id, u.display_name, u.created_at
        FROM user_sessions s
        JOIN users u ON u.id = s.user_id
        WHERE s.token_hash = $1
          AND s.revoked_at IS NULL
          AND s.expires_at > $2
        "#,
    )
    .bind(token_hash)
    .bind(now)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn evm_address_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> anyhow::Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT external_identifier
        FROM user_identities
        WHERE user_id = $1 AND provider = $2
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(EVM_WALLET_PROVIDER)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

pub async fn revoke_session(
    pool: &PgPool,
    token_hash: &str,
    now: DateTime<Utc>,
) -> anyhow::Result<bool> {
    let n = sqlx::query(
        r#"
        UPDATE user_sessions
        SET revoked_at = $2
        WHERE token_hash = $1 AND revoked_at IS NULL
        "#,
    )
    .bind(token_hash)
    .bind(now)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(n == 1)
}

#[cfg(test)]
mod tests {
    use super::NormalizedEvmAddress;

    #[test]
    fn rejects_invalid_addresses() {
        assert!(NormalizedEvmAddress::parse("").is_err());
        assert!(NormalizedEvmAddress::parse("0x123").is_err());
        assert!(NormalizedEvmAddress::parse("not-an-address").is_err());
        assert!(NormalizedEvmAddress::parse("0xzz").is_err());
    }

    #[test]
    fn canonicalizes_case() {
        let mixed = "0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B";
        let lower = "0xab5801a7d398351b8be11c439e05c5b3259aec9b";
        let a = NormalizedEvmAddress::parse(mixed).unwrap();
        let b = NormalizedEvmAddress::parse(lower).unwrap();
        assert_eq!(a.as_stored(), b.as_stored());
        assert_eq!(a.as_stored(), lower);
        assert_eq!(a.alloy(), b.alloy());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn duplicate_session_token_is_unique_violation(pool: sqlx::PgPool) {
        let addr = super::NormalizedEvmAddress::parse(
            "0xab5801a7d398351b8be11c439e05c5b3259aec9b",
        )
        .unwrap();
        let user = super::resolve_or_create_evm_user(&pool, &addr)
            .await
            .unwrap();
        let hash = "aa".repeat(32);
        let exp = chrono::Utc::now() + chrono::Duration::hours(1);
        super::insert_session(&pool, user.id, &hash, exp)
            .await
            .unwrap();
        let err = super::insert_session(&pool, user.id, &hash, exp)
            .await
            .unwrap_err();
        assert!(super::is_unique_violation(&err));
    }
}
