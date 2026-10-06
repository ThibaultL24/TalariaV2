// crates/talaria-api/src/claim_signals.rs
//! Verify user-signed Intuition deposits, then persist Talaria stance + signal history.

use alloy_primitives::{B256, U256};
use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::PgPool;
use talaria_store::{
    evm_address_for_user, get_claim, get_published_claim_intuition, get_signal_by_tx_hash,
    insert_confirmed_signal, InteractionAction, InteractionTargetType, InteractionVisibility,
    IntuitionUserSignalInsert, UserRow,
};
use uuid::Uuid;

use crate::interactions::{create_interaction, InteractionDto, InteractionError};

pub const INTUITION_TESTNET_CHAIN_ID: i64 = 13579;
pub const INTUITION_MAINNET_CHAIN_ID: i64 = 1155;
pub const MULTIVAULT_TESTNET: &str = "0x2ece8d4dedcb9918a398528f3fa4688b1d2cab91";
pub const MULTIVAULT_MAINNET: &str = "0x6e35cf57a41fa15ea0eae9c33e751b01a784fe7e";
/// Deposited(address,address,bytes32,uint256,uint256,uint256,uint256,uint8)
pub const DEPOSITED_TOPIC0: &str =
    "0x35a17feadae5a084f3916aa9b9ad916c3301ec319fb767bf963921f064e84aed";
const GET_INVERSE_SELECTOR: [u8; 4] = [0x62, 0xa8, 0x0f, 0x3d];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalError {
    Unauthenticated,
    ClaimNotFound,
    NotPublished,
    InvalidAction,
    TxNotFound,
    TxReverted,
    WrongChain,
    WrongMultiVault,
    WrongTerm,
    WrongWallet,
    DuplicateTx,
    RpcError,
    DatabaseError,
}

impl SignalError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthenticated => "unauthenticated",
            Self::ClaimNotFound => "claim_not_found",
            Self::NotPublished => "not_published",
            Self::InvalidAction => "invalid_action",
            Self::TxNotFound => "tx_not_found",
            Self::TxReverted => "tx_reverted",
            Self::WrongChain => "wrong_chain",
            Self::WrongMultiVault => "wrong_multivault",
            Self::WrongTerm => "wrong_term",
            Self::WrongWallet => "wrong_wallet",
            Self::DuplicateTx => "duplicate_tx",
            Self::RpcError => "rpc_error",
            Self::DatabaseError => "database_error",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::Unauthenticated => "Not authenticated",
            Self::ClaimNotFound => "Claim not found",
            Self::NotPublished => "Claim is not published on Intuition",
            Self::InvalidAction => "Only support and dispute can sync Intuition deposits",
            Self::TxNotFound => "Transaction hash not found",
            Self::TxReverted => "Transaction reverted",
            Self::WrongChain => "Transaction is not on the configured Intuition chain",
            Self::WrongMultiVault => "Transaction did not target the Intuition MultiVault",
            Self::WrongTerm => "Deposit termId does not match this claim",
            Self::WrongWallet => "Deposit receiver does not match the authenticated wallet",
            Self::DuplicateTx => "This transaction is already linked to another user",
            Self::RpcError => "Could not verify the transaction",
            Self::DatabaseError => "Internal database error",
        }
    }
}

impl From<InteractionError> for SignalError {
    fn from(err: InteractionError) -> Self {
        match err {
            InteractionError::Unauthenticated => Self::Unauthenticated,
            InteractionError::TargetNotFound => Self::ClaimNotFound,
            _ => Self::DatabaseError,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChainTx {
    pub to: Option<String>,
    pub chain_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ChainLog {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
}

#[derive(Debug, Clone)]
pub struct ChainReceipt {
    pub status: u64,
    pub to: Option<String>,
    pub logs: Vec<ChainLog>,
}

#[async_trait]
pub trait IntuitionChain: Send + Sync {
    async fn get_transaction(&self, tx_hash: &str) -> Result<Option<ChainTx>, SignalError>;
    async fn get_receipt(&self, tx_hash: &str) -> Result<Option<ChainReceipt>, SignalError>;
    async fn inverse_triple_id(
        &self,
        vault: &str,
        triple_term_id: &str,
    ) -> Result<String, SignalError>;
}

pub fn configured_chain_id() -> i64 {
    std::env::var("INTUITION_CHAIN_ID")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(INTUITION_TESTNET_CHAIN_ID)
}

pub fn multivault_address(chain_id: i64) -> &'static str {
    if chain_id == INTUITION_MAINNET_CHAIN_ID {
        MULTIVAULT_MAINNET
    } else {
        MULTIVAULT_TESTNET
    }
}

pub fn default_rpc_url(chain_id: i64) -> String {
    if let Ok(url) = std::env::var("INTUITION_RPC_URL") {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if chain_id == INTUITION_MAINNET_CHAIN_ID {
        "https://rpc.intuition.systems/http".into()
    } else {
        "https://testnet.rpc.intuition.systems/http".into()
    }
}

pub struct HttpIntuitionChain {
    pub rpc_url: String,
    pub client: reqwest::Client,
}

impl HttpIntuitionChain {
    pub fn for_chain(chain_id: i64) -> Self {
        Self {
            rpc_url: default_rpc_url(chain_id),
            client: reqwest::Client::new(),
        }
    }

    async fn rpc(&self, method: &str, params: Value) -> Result<Value, SignalError> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        });
        let resp = self
            .client
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|_| SignalError::RpcError)?;
        let v: Value = resp.json().await.map_err(|_| SignalError::RpcError)?;
        if v.get("error").is_some() {
            return Err(SignalError::RpcError);
        }
        Ok(v["result"].clone())
    }
}

#[async_trait]
impl IntuitionChain for HttpIntuitionChain {
    async fn get_transaction(&self, tx_hash: &str) -> Result<Option<ChainTx>, SignalError> {
        let result = self
            .rpc("eth_getTransactionByHash", json!([tx_hash]))
            .await?;
        if result.is_null() {
            return Ok(None);
        }
        Ok(Some(ChainTx {
            to: result["to"].as_str().map(|s| s.to_string()),
            chain_id: parse_hex_u64(result["chainId"].as_str()).map(|n| n as i64),
        }))
    }

    async fn get_receipt(&self, tx_hash: &str) -> Result<Option<ChainReceipt>, SignalError> {
        let result = self
            .rpc("eth_getTransactionReceipt", json!([tx_hash]))
            .await?;
        if result.is_null() {
            return Ok(None);
        }
        let status = parse_hex_u64(result["status"].as_str()).unwrap_or(0);
        let logs = result["logs"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|log| ChainLog {
                address: log["address"].as_str().unwrap_or_default().to_string(),
                topics: log["topics"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|t| t.as_str().map(|s| s.to_string()))
                    .collect(),
                data: log["data"].as_str().unwrap_or("0x").to_string(),
            })
            .collect();
        Ok(Some(ChainReceipt {
            status,
            to: result["to"].as_str().map(|s| s.to_string()),
            logs,
        }))
    }

    async fn inverse_triple_id(
        &self,
        vault: &str,
        triple_term_id: &str,
    ) -> Result<String, SignalError> {
        let term = parse_bytes32(triple_term_id).ok_or(SignalError::WrongTerm)?;
        let mut data = Vec::with_capacity(36);
        data.extend_from_slice(&GET_INVERSE_SELECTOR);
        data.extend_from_slice(term.as_slice());
        let result = self
            .rpc(
                "eth_call",
                json!([
                    { "to": vault, "data": format!("0x{}", hex::encode(data)) },
                    "latest"
                ]),
            )
            .await?;
        let hex = result.as_str().ok_or(SignalError::RpcError)?;
        let bytes = parse_bytes32(hex).ok_or(SignalError::RpcError)?;
        Ok(format!("0x{}", hex::encode(bytes)))
    }
}

fn parse_hex_u64(raw: Option<&str>) -> Option<u64> {
    let s = raw?.trim();
    let s = s.strip_prefix("0x").unwrap_or(s);
    u64::from_str_radix(s, 16).ok()
}

fn parse_bytes32(raw: &str) -> Option<B256> {
    let hex = raw.trim().strip_prefix("0x").unwrap_or(raw.trim());
    if hex.len() != 64 {
        return None;
    }
    let bytes = hex::decode(hex).ok()?;
    B256::try_from(bytes.as_slice()).ok()
}

fn normalize_hex(raw: &str) -> String {
    let t = raw.trim().to_lowercase();
    if t.starts_with("0x") {
        t
    } else {
        format!("0x{t}")
    }
}

fn last40_hex(raw: &str) -> String {
    let hex: String = raw
        .trim()
        .trim_start_matches("0x")
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if hex.len() >= 40 {
        hex[hex.len() - 40..].to_string()
    } else {
        format!("{hex:0>40}")
    }
}

fn addresses_equal(a: &str, b: &str) -> bool {
    last40_hex(a) == last40_hex(b)
}

fn topic_address(topic: &str) -> Option<String> {
    Some(format!("0x{}", last40_hex(topic)))
}

#[derive(Debug, Clone)]
pub struct DepositedEvent {
    pub receiver: String,
    pub term_id: String,
    pub curve_id: String,
    pub assets: String,
    pub shares: String,
}

pub fn parse_deposited_logs(logs: &[ChainLog], vault: &str) -> Option<DepositedEvent> {
    for log in logs {
        if !addresses_equal(&log.address, vault) {
            continue;
        }
        if log.topics.len() < 4 {
            continue;
        }
        if normalize_hex(&log.topics[0]) != DEPOSITED_TOPIC0 {
            continue;
        }
        let receiver = topic_address(&log.topics[2])?;
        let term_id = normalize_hex(&log.topics[3]);
        let data = hex::decode(log.data.trim().trim_start_matches("0x")).ok()?;
        if data.len() < 32 * 4 {
            continue;
        }
        let curve_id = U256::from_be_slice(&data[0..32]).to_string();
        let assets = U256::from_be_slice(&data[32..64]).to_string();
        let shares = U256::from_be_slice(&data[96..128]).to_string();
        return Some(DepositedEvent {
            receiver,
            term_id,
            curve_id,
            assets,
            shares,
        });
    }
    None
}

pub fn parse_signal_action(raw: &str) -> Result<InteractionAction, SignalError> {
    match raw {
        "support" => Ok(InteractionAction::Support),
        "dispute" => Ok(InteractionAction::Dispute),
        _ => Err(SignalError::InvalidAction),
    }
}

pub fn normalize_tx_hash(raw: &str) -> Result<String, SignalError> {
    let hex = raw.trim().strip_prefix("0x").unwrap_or(raw.trim());
    if hex.len() != 64 || hex::decode(hex).is_err() {
        return Err(SignalError::TxNotFound);
    }
    Ok(format!("0x{}", hex.to_lowercase()))
}

pub fn encode_topic_address(addr: &str) -> String {
    format!("0x{}", format!("{:0>64}", last40_hex(addr)))
}

pub fn encode_uint256(n: u64) -> Vec<u8> {
    let mut out = vec![0u8; 32];
    out[24..].copy_from_slice(&n.to_be_bytes());
    out
}

pub async fn claim_intuition_status(pool: &PgPool, claim_id: Uuid) -> Result<Value, SignalError> {
    if get_claim(pool, claim_id)
        .await
        .map_err(|_| SignalError::DatabaseError)?
        .is_none()
    {
        return Err(SignalError::ClaimNotFound);
    }
    match get_published_claim_intuition(pool, claim_id)
        .await
        .map_err(|_| SignalError::DatabaseError)?
    {
        Some(binding) => Ok(json!({
            "status": "ready",
            "claim_id": claim_id,
            "chain_id": binding.chain_id,
            "triple_term_id": binding.triple_term_id,
            "counter_term_id": Value::Null
        })),
        None => Ok(json!({
            "status": "not_published",
            "claim_id": claim_id
        })),
    }
}

pub async fn sync_claim_signal<C: IntuitionChain>(
    pool: &PgPool,
    chain: &C,
    user: &UserRow,
    claim_id: Uuid,
    action: InteractionAction,
    tx_hash_raw: &str,
) -> Result<(InteractionDto, bool), SignalError> {
    if !matches!(
        action,
        InteractionAction::Support | InteractionAction::Dispute
    ) {
        return Err(SignalError::InvalidAction);
    }
    let tx_hash = normalize_tx_hash(tx_hash_raw)?;
    if get_claim(pool, claim_id)
        .await
        .map_err(|_| SignalError::DatabaseError)?
        .is_none()
    {
        return Err(SignalError::ClaimNotFound);
    }
    let wallet = evm_address_for_user(pool, user.id)
        .await
        .map_err(|_| SignalError::DatabaseError)?
        .ok_or(SignalError::WrongWallet)?;

    if let Some(existing) = get_signal_by_tx_hash(pool, &tx_hash)
        .await
        .map_err(|_| SignalError::DatabaseError)?
    {
        if existing.user_id != user.id {
            return Err(SignalError::DuplicateTx);
        }
        let created = create_interaction(
            pool,
            user.id,
            action,
            InteractionTargetType::Claim,
            claim_id,
            Some(InteractionVisibility::Public),
        )
        .await?;
        let dto = InteractionDto::try_from(created.row).map_err(|_| SignalError::DatabaseError)?;
        return Ok((dto, false));
    }

    let binding = get_published_claim_intuition(pool, claim_id)
        .await
        .map_err(|_| SignalError::DatabaseError)?
        .ok_or(SignalError::NotPublished)?;
    let chain_id = if binding.chain_id == 0 {
        configured_chain_id()
    } else {
        i64::from(binding.chain_id)
    };
    let vault = multivault_address(chain_id);

    let tx = chain
        .get_transaction(&tx_hash)
        .await?
        .ok_or(SignalError::TxNotFound)?;
    if let Some(tx_chain) = tx.chain_id {
        if tx_chain != chain_id {
            return Err(SignalError::WrongChain);
        }
    }
    let to = tx.to.as_deref().ok_or(SignalError::WrongMultiVault)?;
    if !addresses_equal(to, vault) {
        return Err(SignalError::WrongMultiVault);
    }

    let receipt = chain
        .get_receipt(&tx_hash)
        .await?
        .ok_or(SignalError::TxNotFound)?;
    if receipt.status != 1 {
        return Err(SignalError::TxReverted);
    }
    if let Some(rto) = receipt.to.as_deref() {
        if !addresses_equal(rto, vault) {
            return Err(SignalError::WrongMultiVault);
        }
    }

    let event = parse_deposited_logs(&receipt.logs, vault).ok_or(SignalError::WrongMultiVault)?;
    if !addresses_equal(&event.receiver, &wallet) {
        return Err(SignalError::WrongWallet);
    }

    let expected_term = match action {
        InteractionAction::Support => normalize_hex(&binding.triple_term_id),
        InteractionAction::Dispute => normalize_hex(
            &chain
                .inverse_triple_id(vault, &binding.triple_term_id)
                .await?,
        ),
        _ => return Err(SignalError::InvalidAction),
    };
    if event.term_id != expected_term {
        return Err(SignalError::WrongTerm);
    }

    let created = create_interaction(
        pool,
        user.id,
        action,
        InteractionTargetType::Claim,
        claim_id,
        Some(InteractionVisibility::Public),
    )
    .await?;
    let signal = insert_confirmed_signal(
        pool,
        &IntuitionUserSignalInsert {
            user_id: user.id,
            claim_id,
            interaction_id: Some(created.row.id),
            action_type: action.as_str().to_string(),
            chain_id,
            term_id: event.term_id,
            curve_id: event.curve_id,
            tx_hash: tx_hash.clone(),
            assets: event.assets,
            shares: Some(event.shares),
            receiver: format!("0x{}", last40_hex(&event.receiver)),
        },
    )
    .await
    .map_err(|_| SignalError::DatabaseError)?;
    if signal.user_id != user.id {
        return Err(SignalError::DuplicateTx);
    }
    let created_flag = created.created;
    let dto = InteractionDto::try_from(created.row).map_err(|_| SignalError::DatabaseError)?;
    Ok((dto, created_flag))
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn parses_deposited_log() {
        let vault = MULTIVAULT_TESTNET;
        let receiver = "0xab5801a7d398351b8be11c439e05c5b3259aec9b";
        let term = "0x1111111111111111111111111111111111111111111111111111111111111111";
        let mut data = Vec::new();
        data.extend(encode_uint256(2));
        data.extend(encode_uint256(10));
        data.extend(encode_uint256(9));
        data.extend(encode_uint256(7));
        let log = ChainLog {
            address: vault.to_string(),
            topics: vec![
                DEPOSITED_TOPIC0.to_string(),
                encode_topic_address(receiver),
                encode_topic_address(receiver),
                term.to_string(),
            ],
            data: format!("0x{}", hex::encode(data)),
        };
        let ev = parse_deposited_logs(&[log], vault).unwrap();
        assert_eq!(ev.curve_id, "2");
        assert_eq!(ev.assets, "10");
        assert_eq!(ev.shares, "7");
        assert_eq!(last40_hex(&ev.receiver), last40_hex(receiver));
        assert_eq!(ev.term_id, term);
    }
}

#[cfg(test)]
mod sync_tests {
    use super::*;
    use talaria_store::{
        insert_claim, resolve_or_create_evm_user, upsert_entity_with_kind, ClaimInsert,
        NormalizedEvmAddress,
    };

    const WALLET: &str = "0xab5801a7d398351b8be11c439e05c5b3259aec9b";
    const TRIPLE: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const COUNTER: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const TX: &str = "0xcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    struct MockChain {
        tx: Option<ChainTx>,
        receipt: Option<ChainReceipt>,
        inverse: String,
    }

    #[async_trait]
    impl IntuitionChain for MockChain {
        async fn get_transaction(&self, _: &str) -> Result<Option<ChainTx>, SignalError> {
            Ok(self.tx.clone())
        }
        async fn get_receipt(&self, _: &str) -> Result<Option<ChainReceipt>, SignalError> {
            Ok(self.receipt.clone())
        }
        async fn inverse_triple_id(&self, _: &str, _: &str) -> Result<String, SignalError> {
            Ok(self.inverse.clone())
        }
    }

    fn deposited_receipt(receiver: &str, term: &str, vault: &str, status: u64) -> ChainReceipt {
        let mut data = Vec::new();
        data.extend(encode_uint256(1));
        data.extend(encode_uint256(1_000));
        data.extend(encode_uint256(900));
        data.extend(encode_uint256(50));
        ChainReceipt {
            status,
            to: Some(vault.to_string()),
            logs: vec![ChainLog {
                address: vault.to_string(),
                topics: vec![
                    DEPOSITED_TOPIC0.to_string(),
                    encode_topic_address(receiver),
                    encode_topic_address(receiver),
                    term.to_string(),
                ],
                data: format!("0x{}", hex::encode(data)),
            }],
        }
    }

    fn ok_chain(term: &str) -> MockChain {
        MockChain {
            tx: Some(ChainTx {
                to: Some(MULTIVAULT_TESTNET.to_string()),
                chain_id: Some(13579),
            }),
            receipt: Some(deposited_receipt(WALLET, term, MULTIVAULT_TESTNET, 1)),
            inverse: COUNTER.to_string(),
        }
    }

    async fn seed_user(pool: &sqlx::PgPool) -> UserRow {
        resolve_or_create_evm_user(pool, &NormalizedEvmAddress::parse(WALLET).unwrap())
            .await
            .unwrap()
    }

    async fn seed_claim(pool: &sqlx::PgPool, published: bool) -> Uuid {
        let entity = upsert_entity_with_kind(pool, "en", "PR4 Person", "person")
            .await
            .unwrap();
        let claim = insert_claim(
            pool,
            &ClaimInsert {
                entity_id: entity,
                claim_kind: "theory".into(),
                text: "PR4 theory".into(),
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
        .unwrap();
        if published {
            sqlx::query(
                r#"
                INSERT INTO intuition_publications (
                    subject_entity_id, debate_id, bundle_fingerprint, kind, status,
                    payload_json, soft_claim_id, chain_id, triple_term_id
                ) VALUES ($1, 'd', $2, 'theory', 'published', '{}'::jsonb, $3, 13579, $4)
                "#,
            )
            .bind(entity)
            .bind(format!("fp-{claim}"))
            .bind(claim)
            .bind(TRIPLE)
            .execute(pool)
            .await
            .unwrap();
        }
        claim
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn get_not_published(pool: sqlx::PgPool) {
        let claim = seed_claim(&pool, false).await;
        let body = claim_intuition_status(&pool, claim).await.unwrap();
        assert_eq!(body["status"], "not_published");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn get_ready(pool: sqlx::PgPool) {
        let claim = seed_claim(&pool, true).await;
        let body = claim_intuition_status(&pool, claim).await.unwrap();
        assert_eq!(body["status"], "ready");
        assert_eq!(body["triple_term_id"], TRIPLE);
        assert!(body["counter_term_id"].is_null());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn sync_unknown_tx(pool: sqlx::PgPool) {
        let user = seed_user(&pool).await;
        let claim = seed_claim(&pool, true).await;
        let chain = MockChain {
            tx: None,
            receipt: None,
            inverse: COUNTER.into(),
        };
        let err = sync_claim_signal(&pool, &chain, &user, claim, InteractionAction::Support, TX)
            .await
            .unwrap_err();
        assert_eq!(err, SignalError::TxNotFound);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn sync_reverted(pool: sqlx::PgPool) {
        let user = seed_user(&pool).await;
        let claim = seed_claim(&pool, true).await;
        let mut chain = ok_chain(TRIPLE);
        chain.receipt = Some(deposited_receipt(WALLET, TRIPLE, MULTIVAULT_TESTNET, 0));
        let err = sync_claim_signal(&pool, &chain, &user, claim, InteractionAction::Support, TX)
            .await
            .unwrap_err();
        assert_eq!(err, SignalError::TxReverted);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn sync_wrong_chain(pool: sqlx::PgPool) {
        let user = seed_user(&pool).await;
        let claim = seed_claim(&pool, true).await;
        let mut chain = ok_chain(TRIPLE);
        chain.tx = Some(ChainTx {
            to: Some(MULTIVAULT_TESTNET.into()),
            chain_id: Some(1),
        });
        let err = sync_claim_signal(&pool, &chain, &user, claim, InteractionAction::Support, TX)
            .await
            .unwrap_err();
        assert_eq!(err, SignalError::WrongChain);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn sync_wrong_vault(pool: sqlx::PgPool) {
        let user = seed_user(&pool).await;
        let claim = seed_claim(&pool, true).await;
        let mut chain = ok_chain(TRIPLE);
        chain.tx = Some(ChainTx {
            to: Some("0x0000000000000000000000000000000000000001".into()),
            chain_id: Some(13579),
        });
        let err = sync_claim_signal(&pool, &chain, &user, claim, InteractionAction::Support, TX)
            .await
            .unwrap_err();
        assert_eq!(err, SignalError::WrongMultiVault);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn sync_wrong_term(pool: sqlx::PgPool) {
        let user = seed_user(&pool).await;
        let claim = seed_claim(&pool, true).await;
        let chain = ok_chain(COUNTER);
        let err = sync_claim_signal(&pool, &chain, &user, claim, InteractionAction::Support, TX)
            .await
            .unwrap_err();
        assert_eq!(err, SignalError::WrongTerm);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn sync_wrong_wallet(pool: sqlx::PgPool) {
        let user = seed_user(&pool).await;
        let claim = seed_claim(&pool, true).await;
        let mut chain = ok_chain(TRIPLE);
        chain.receipt = Some(deposited_receipt(
            "0x1111111111111111111111111111111111111111",
            TRIPLE,
            MULTIVAULT_TESTNET,
            1,
        ));
        let err = sync_claim_signal(&pool, &chain, &user, claim, InteractionAction::Support, TX)
            .await
            .unwrap_err();
        assert_eq!(err, SignalError::WrongWallet);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn sync_support_creates_interaction(pool: sqlx::PgPool) {
        let user = seed_user(&pool).await;
        let claim = seed_claim(&pool, true).await;
        let chain = ok_chain(TRIPLE);
        let (dto, created) =
            sync_claim_signal(&pool, &chain, &user, claim, InteractionAction::Support, TX)
                .await
                .unwrap();
        assert!(created);
        assert_eq!(dto.action, InteractionAction::Support);
        let again =
            sync_claim_signal(&pool, &chain, &user, claim, InteractionAction::Support, TX)
                .await
                .unwrap();
        assert!(!again.1);
        assert_eq!(again.0.id, dto.id);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn sync_dispute_and_history(pool: sqlx::PgPool) {
        let user = seed_user(&pool).await;
        let claim = seed_claim(&pool, true).await;
        let support_tx = TX;
        let dispute_tx = "0xdddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
        let (support, _) = sync_claim_signal(
            &pool,
            &ok_chain(TRIPLE),
            &user,
            claim,
            InteractionAction::Support,
            support_tx,
        )
        .await
        .unwrap();
        let (dispute, _) = sync_claim_signal(
            &pool,
            &ok_chain(COUNTER),
            &user,
            claim,
            InteractionAction::Dispute,
            dispute_tx,
        )
        .await
        .unwrap();
        assert_eq!(dispute.action, InteractionAction::Dispute);
        assert_ne!(support.id, dispute.id);
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM intuition_user_signals WHERE user_id = $1 AND claim_id = $2",
        )
        .bind(user.id)
        .bind(claim)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, 2);
        let stance: String = sqlx::query_scalar(
            r#"
            SELECT action_type FROM user_interactions
            WHERE user_id = $1 AND target_id = $2
              AND action_type IN ('support','dispute','uncertain')
            "#,
        )
        .bind(user.id)
        .bind(claim)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(stance, "dispute");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn duplicate_tx_other_user(pool: sqlx::PgPool) {
        let user = seed_user(&pool).await;
        let claim = seed_claim(&pool, true).await;
        sync_claim_signal(
            &pool,
            &ok_chain(TRIPLE),
            &user,
            claim,
            InteractionAction::Support,
            TX,
        )
        .await
        .unwrap();
        let other = resolve_or_create_evm_user(
            &pool,
            &NormalizedEvmAddress::parse("0x1111111111111111111111111111111111111111").unwrap(),
        )
        .await
        .unwrap();
        let err = sync_claim_signal(
            &pool,
            &ok_chain(TRIPLE),
            &other,
            claim,
            InteractionAction::Support,
            TX,
        )
        .await
        .unwrap_err();
        assert_eq!(err, SignalError::DuplicateTx);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn unpublished_rejects_sync(pool: sqlx::PgPool) {
        let user = seed_user(&pool).await;
        let claim = seed_claim(&pool, false).await;
        let err = sync_claim_signal(
            &pool,
            &ok_chain(TRIPLE),
            &user,
            claim,
            InteractionAction::Support,
            TX,
        )
        .await
        .unwrap_err();
        assert_eq!(err, SignalError::NotPublished);
    }
}
