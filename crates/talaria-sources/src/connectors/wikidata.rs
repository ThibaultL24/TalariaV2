// crates/talaria-sources/src/connectors/wikidata.rs
use async_trait::async_trait;
use serde_json::Value;

use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::kinds::{DiscoveryMethod, DocumentType, SourceKind};
use crate::plan::ResolvedSubject;
use crate::types::{DiscoveredDocument, ExternalEntityRef, SourceMetadata};

const UA: &str = "TalariaEngine/0.1 (https://github.com/talaria; multi-source ingest)";

#[derive(Debug, Clone)]
pub struct WikidataSourceConnectorConfig {
    pub api_base: String,
}

impl Default for WikidataSourceConnectorConfig {
    fn default() -> Self {
        Self {
            api_base: "https://www.wikidata.org/w/api.php".into(),
        }
    }
}

pub struct WikidataSourceConnector {
    http: reqwest::Client,
    config: WikidataSourceConnectorConfig,
}

impl WikidataSourceConnector {
    pub fn new(config: WikidataSourceConnectorConfig) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder().user_agent(UA).build()?;
        Ok(Self { http, config })
    }

    async fn resolve_qid(&self, subject: &ResolvedSubject) -> Result<String, ConnectorError> {
        if let Some(qid) = &subject.qid {
            return Ok(qid.clone());
        }
        let response = self
            .http
            .get(&self.config.api_base)
            .query(&[
                ("action", "wbsearchentities"),
                ("search", subject.label.as_str()),
                ("language", "en"),
                ("format", "json"),
                ("limit", "1"),
            ])
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .error_for_status()
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .json::<Value>()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))?;
        response
            .pointer("/search/0/id")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| ConnectorError::Parse("no wikidata hit".into()))
    }

    fn statements_to_text(entity: &Value) -> String {
        let parsed = talaria_wikidata::parse_entity_claims(entity);
        talaria_wikidata::promoted_statement_lines(&parsed)
    }
}

#[async_trait]
impl SourceConnector for WikidataSourceConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::Wikidata
    }

    fn connector_version(&self) -> &str {
        "wikidata:statements_v1"
    }

    async fn discover(
        &self,
        subject: &ResolvedSubject,
        cursor: Option<DiscoveryCursor>,
    ) -> Result<DiscoveryPage, ConnectorError> {
        if cursor.map(|c| c.offset).unwrap_or(0) > 0 {
            return Ok(DiscoveryPage {
                documents: vec![],
                next_cursor: None,
            });
        }
        let qid = self.resolve_qid(subject).await?;
        Ok(DiscoveryPage {
            documents: vec![DiscoveredDocument {
                source_kind: SourceKind::Wikidata,
                external_id: qid.clone(),
                canonical_url: Some(format!("https://www.wikidata.org/wiki/{qid}")),
                title: format!("{} ({qid})", subject.label),
                language: Some("en".into()),
                document_type: DocumentType::StructuredStatement,
                subject_links: vec![ExternalEntityRef {
                    system: "wikidata".into(),
                    id: qid,
                    label: Some(subject.label.clone()),
                }],
                publication_time: None,
                discovery_method: DiscoveryMethod::IdentifierLookup,
                relevance_score: 0.99,
                source_metadata: SourceMetadata::default(),
            }],
            next_cursor: None,
        })
    }

    async fn fetch(
        &self,
        document: &DiscoveredDocument,
    ) -> Result<FetchedDocument, ConnectorError> {
        let qid = &document.external_id;
        let response = self
            .http
            .get(&self.config.api_base)
            .query(&[
                ("action", "wbgetentities"),
                ("ids", qid.as_str()),
                ("props", "claims|labels|sitelinks"),
                ("languages", "en|fr"),
                ("format", "json"),
            ])
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .error_for_status()
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .json::<Value>()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))?;

        let entity = response
            .pointer(&format!("/entities/{qid}"))
            .cloned()
            .ok_or_else(|| ConnectorError::Parse("missing entity".into()))?;
        let text = Self::statements_to_text(&entity);
        let lastrevid = entity
            .get("lastrevid")
            .and_then(|v| v.as_u64())
            .map(|n| n.to_string());

        Ok(FetchedDocument {
            discovered: document.clone(),
            revision_id: lastrevid,
            content_type: "application/vnd.wikibase.entity+json".into(),
            text,
            raw_metadata: entity,
            license: Some("CC0".into()),
            content_bytes: document.external_id.len() as u64,
        })
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        Ok(ConnectorHealth {
            ok: true,
            detail: "wikidata api client ready".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn statements_text_is_promoted_lines_only() {
        let entity = json!({
            "id": "Q517",
            "lastrevid": 1,
            "claims": {
                "P551": [{
                    "id": "Q517$p551",
                    "rank": "normal",
                    "mainsnak": {
                        "snaktype": "value",
                        "property": "P551",
                        "datavalue": { "value": { "id": "Q90" } }
                    },
                    "qualifiers": {
                        "P580": [{
                            "datavalue": { "value": { "time": "+1804-01-01T00:00:00Z", "precision": 9 } }
                        }]
                    }
                }],
                "P106": [{
                    "id": "Q517$p106",
                    "rank": "normal",
                    "mainsnak": {
                        "snaktype": "value",
                        "property": "P106",
                        "datavalue": { "value": { "id": "Q82955" } }
                    }
                }]
            }
        });
        let text = WikidataSourceConnector::statements_to_text(&entity);
        assert_eq!(text, "STATEMENT\tresidence\tresided_in\t1804\tQ90");
    }
}
