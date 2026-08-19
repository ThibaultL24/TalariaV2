// crates/talaria-sources/src/connectors/europeana.rs
use async_trait::async_trait;
use serde_json::Value;

use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::connectors::catalog::{
    bibliographic_notice, http_client, json_first_string, names_match, parse_year, year_in_life,
    NoticeRelation,
};
use crate::kinds::{DiscoveryMethod, DocumentType, SourceKind};
use crate::plan::ResolvedSubject;
use crate::types::{DiscoveredDocument, ExternalEntityRef, SourceMetadata};

const SEARCH: &str = "https://api.europeana.eu/record/v2/search.json";

pub struct EuropeanaConnector {
    http: reqwest::Client,
    api_key: String,
    max_docs: u32,
}

impl EuropeanaConnector {
    pub fn from_env() -> Result<Self, ConnectorError> {
        let api_key = std::env::var("EUROPEANA_API_KEY")
            .map_err(|_| ConnectorError::NotConfigured("EUROPEANA_API_KEY".into()))?;
        if api_key.trim().is_empty() {
            return Err(ConnectorError::NotConfigured("EUROPEANA_API_KEY".into()));
        }
        Ok(Self {
            http: http_client()?,
            api_key,
            max_docs: 20,
        })
    }

    pub fn parse_search(subject: &ResolvedSubject, payload: &Value) -> Vec<DiscoveredDocument> {
        let Some(items) = payload.get("items").and_then(|v| v.as_array()) else {
            return vec![];
        };
        let mut out = Vec::new();
        for item in items {
            let id = item
                .get("id")
                .and_then(json_first_string)
                .unwrap_or_default();
            if id.is_empty() {
                continue;
            }
            let title = item
                .get("title")
                .and_then(json_first_string)
                .unwrap_or_else(|| "Untitled".into());
            let year = item
                .get("year")
                .and_then(json_first_string)
                .and_then(|s| parse_year(&s));
            let who = item.get("dcCreator").and_then(json_first_string);
            let authored = who
                .as_deref()
                .map(|name| names_match(&subject.label, name))
                .unwrap_or(false);
            if authored {
                if let Some(year) = year {
                    if !year_in_life(year, subject) {
                        continue;
                    }
                }
            }
            let place = item
                .get("edmPlaceLabel")
                .and_then(json_first_string)
                .or_else(|| item.get("country").and_then(json_first_string));
            let description = item
                .get("dcDescription")
                .and_then(json_first_string)
                .or_else(|| item.get("description").and_then(json_first_string));
            let shown_at = item.get("edmIsShownAt").and_then(json_first_string);
            let relation = if authored {
                NoticeRelation::Authored
            } else {
                NoticeRelation::About
            };
            let text = bibliographic_notice(
                &subject.label,
                &title,
                year,
                place.as_deref(),
                description.as_deref(),
                relation,
            );
            out.push(DiscoveredDocument {
                source_kind: SourceKind::Europeana,
                external_id: id.clone(),
                canonical_url: shown_at.or_else(|| {
                    Some(format!("https://www.europeana.eu/item{id}"))
                }),
                title,
                language: item.get("language").and_then(json_first_string),
                document_type: DocumentType::BibliographicNotice,
                subject_links: vec![ExternalEntityRef {
                    system: "europeana".into(),
                    id,
                    label: Some(subject.label.clone()),
                }],
                publication_time: year.map(|y| crate::types::TypedTimeLite::Exact {
                    year: y,
                    surface: Some(y.to_string()),
                }),
                discovery_method: DiscoveryMethod::CatalogSearch,
                relevance_score: if authored { 0.78 } else { 0.5 },
                source_metadata: SourceMetadata {
                    raw: serde_json::json!({ "notice": text, "who": who, "place": place }),
                },
            });
        }
        out
    }
}

#[async_trait]
impl SourceConnector for EuropeanaConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::Europeana
    }

    fn connector_version(&self) -> &str {
        "europeana:search_v2"
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
        let query = format!("\"{}\"", subject.label);
        let rows = self.max_docs.to_string();
        let response = self
            .http
            .get(SEARCH)
            .query(&[
                ("wskey", self.api_key.as_str()),
                ("query", query.as_str()),
                ("rows", rows.as_str()),
                ("profile", "rich"),
            ])
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .error_for_status()
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .json::<Value>()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))?;
        if response.get("success").and_then(|v| v.as_bool()) == Some(false) {
            return Err(ConnectorError::Http(
                json_first_string(&response).unwrap_or_else(|| "europeana search failed".into()),
            ));
        }
        Ok(DiscoveryPage {
            documents: Self::parse_search(subject, &response),
            next_cursor: None,
        })
    }

    async fn fetch(
        &self,
        document: &DiscoveredDocument,
    ) -> Result<FetchedDocument, ConnectorError> {
        let text = document
            .source_metadata
            .raw
            .get("notice")
            .and_then(|v| v.as_str())
            .unwrap_or(&document.title)
            .to_string();
        Ok(FetchedDocument {
            discovered: document.clone(),
            revision_id: None,
            content_type: "text/plain".into(),
            content_bytes: text.len() as u64,
            raw_metadata: document.source_metadata.raw.clone(),
            license: Some("Europeana item rights as provided".into()),
            text,
        })
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        Ok(ConnectorHealth {
            ok: true,
            detail: "search.json with EUROPEANA_API_KEY".into(),
        })
    }
}
