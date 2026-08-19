// crates/talaria-sources/src/connectors/open_library.rs
use async_trait::async_trait;
use serde_json::Value;

use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::connectors::catalog::{
    bibliographic_notice, catalog_place, http_client, json_first_string, names_match, parse_year,
    year_in_life, NoticeRelation,
};
use crate::kinds::{DiscoveryMethod, DocumentType, SourceKind};
use crate::plan::ResolvedSubject;
use crate::types::{DiscoveredDocument, ExternalEntityRef, SourceMetadata};

const SEARCH: &str = "https://openlibrary.org/search.json";

pub struct OpenLibraryConnector {
    http: reqwest::Client,
    max_docs: u32,
}

impl OpenLibraryConnector {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            http: http_client()?,
            max_docs: 20,
        })
    }

    pub fn parse_search(subject: &ResolvedSubject, payload: &Value) -> Vec<DiscoveredDocument> {
        let Some(docs) = payload.get("docs").and_then(|v| v.as_array()) else {
            return vec![];
        };
        let mut out = Vec::new();
        for doc in docs {
            let title = doc
                .get("title")
                .and_then(json_first_string)
                .unwrap_or_else(|| "Untitled".into());
            let key = doc
                .get("key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if key.is_empty() {
                continue;
            }
            let year = doc
                .get("first_publish_year")
                .and_then(|v| v.as_i64())
                .map(|y| y as i32)
                .or_else(|| {
                    doc.get("publish_year")
                        .and_then(json_first_string)
                        .and_then(|s| parse_year(&s))
                });
            let authors: Vec<String> = doc
                .get("author_name")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(json_first_string)
                        .collect()
                })
                .unwrap_or_default();
            let authored = authors.iter().any(|name| names_match(&subject.label, name));
            if authored {
                if let Some(year) = year {
                    if !year_in_life(year, subject) {
                        continue;
                    }
                }
            }
            let place = catalog_place(
                doc.get("publish_place")
                    .and_then(json_first_string)
                    .or_else(|| doc.get("place").and_then(json_first_string))
                    .as_deref(),
            );
            let relation = if authored {
                NoticeRelation::Authored
            } else {
                NoticeRelation::About
            };
            let description = doc.get("subtitle").and_then(json_first_string);
            let text = bibliographic_notice(
                &subject.label,
                &title,
                year,
                place.as_deref(),
                description.as_deref(),
                relation,
            );
            out.push(DiscoveredDocument {
                source_kind: SourceKind::OpenLibrary,
                external_id: key.clone(),
                canonical_url: Some(format!("https://openlibrary.org{key}")),
                title,
                language: doc.get("language").and_then(json_first_string),
                document_type: DocumentType::BibliographicNotice,
                subject_links: vec![ExternalEntityRef {
                    system: "open_library".into(),
                    id: key,
                    label: Some(subject.label.clone()),
                }],
                publication_time: year.map(|y| crate::types::TypedTimeLite::Exact {
                    year: y,
                    surface: Some(y.to_string()),
                }),
                discovery_method: DiscoveryMethod::CatalogSearch,
                relevance_score: if authored { 0.82 } else { 0.55 },
                source_metadata: SourceMetadata {
                    raw: serde_json::json!({
                        "notice": text,
                        "authors": authors,
                        "place": place,
                    }),
                },
            });
        }
        out
    }
}

#[async_trait]
impl SourceConnector for OpenLibraryConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::OpenLibrary
    }

    fn connector_version(&self) -> &str {
        "open_library:search_v1"
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
        let response = self
            .http
            .get(SEARCH)
            .query(&[
                ("q", subject.label.as_str()),
                ("limit", &self.max_docs.to_string()),
                ("fields", "key,title,author_name,first_publish_year,publish_place,language,subtitle"),
            ])
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .error_for_status()
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .json::<Value>()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))?;
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
            license: Some("Open Library data".into()),
            text,
        })
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        Ok(ConnectorHealth {
            ok: true,
            detail: "public search.json".into(),
        })
    }
}
