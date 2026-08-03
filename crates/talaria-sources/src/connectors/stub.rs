// crates/talaria-sources/src/connectors/stub.rs
use async_trait::async_trait;

use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::kinds::SourceKind;
use crate::plan::ResolvedSubject;
use crate::types::DiscoveredDocument;

/// Placeholder connector — interface only, never invents results.
pub struct StubConnector {
    kind: SourceKind,
    reason: String,
}

impl StubConnector {
    pub fn new(kind: SourceKind, reason: impl Into<String>) -> Self {
        Self {
            kind,
            reason: reason.into(),
        }
    }
}

#[async_trait]
impl SourceConnector for StubConnector {
    fn source_kind(&self) -> SourceKind {
        self.kind.clone()
    }

    fn connector_version(&self) -> &str {
        "stub:v0"
    }

    async fn discover(
        &self,
        _subject: &ResolvedSubject,
        _cursor: Option<DiscoveryCursor>,
    ) -> Result<DiscoveryPage, ConnectorError> {
        Err(ConnectorError::NotConfigured(self.reason.clone()))
    }

    async fn fetch(
        &self,
        _document: &DiscoveredDocument,
    ) -> Result<FetchedDocument, ConnectorError> {
        Err(ConnectorError::NotConfigured(self.reason.clone()))
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        Ok(ConnectorHealth {
            ok: false,
            detail: self.reason.clone(),
        })
    }
}
