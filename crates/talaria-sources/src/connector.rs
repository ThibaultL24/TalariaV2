// crates/talaria-sources/src/connector.rs
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::kinds::SourceKind;
use crate::plan::ResolvedSubject;
use crate::types::DiscoveredDocument;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscoveryCursor {
    pub token: Option<String>,
    pub offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryPage {
    pub documents: Vec<DiscoveredDocument>,
    pub next_cursor: Option<DiscoveryCursor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchedDocument {
    pub discovered: DiscoveredDocument,
    pub revision_id: Option<String>,
    pub content_type: String,
    pub text: String,
    pub raw_metadata: serde_json::Value,
    pub license: Option<String>,
    pub content_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorHealth {
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Error)]
pub enum ConnectorError {
    #[error("not configured: {0}")]
    NotConfigured(String),
    #[error("rate limited")]
    RateLimited,
    #[error("http error: {0}")]
    Http(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[async_trait]
pub trait SourceConnector: Send + Sync {
    fn source_kind(&self) -> SourceKind;
    fn connector_version(&self) -> &str;

    async fn discover(
        &self,
        subject: &ResolvedSubject,
        cursor: Option<DiscoveryCursor>,
    ) -> Result<DiscoveryPage, ConnectorError>;

    async fn fetch(
        &self,
        document: &DiscoveredDocument,
    ) -> Result<FetchedDocument, ConnectorError>;

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError>;
}
