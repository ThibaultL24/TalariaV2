// crates/talaria-sources/src/connectors/internet_archive.rs
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

const SEARCH: &str = "https://archive.org/advancedsearch.php";

pub struct InternetArchiveConnector {
    http: reqwest::Client,
    max_docs: u32,
}

impl InternetArchiveConnector {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            http: http_client()?,
            max_docs: 20,
        })
    }

    pub fn parse_search(subject: &ResolvedSubject, payload: &Value) -> Vec<DiscoveredDocument> {
        let Some(docs) = payload
            .pointer("/response/docs")
            .and_then(|v| v.as_array())
        else {
            return vec![];
        };
        let mut out = Vec::new();
        for doc in docs {
            let identifier = doc
                .get("identifier")
                .and_then(json_first_string)
                .unwrap_or_default();
            if identifier.is_empty() {
                continue;
            }
            let title = doc
                .get("title")
                .and_then(json_first_string)
                .unwrap_or_else(|| identifier.clone());
            let year = doc
                .get("year")
                .and_then(json_first_string)
                .and_then(|s| parse_year(&s));
            let creator = doc.get("creator").and_then(json_first_string);
            let authored = creator
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
            let description = doc.get("description").and_then(json_first_string);
            let place = catalog_place(doc.get("place").and_then(json_first_string).as_deref())
                .or_else(|| catalog_place(description.as_deref()));
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
                source_kind: SourceKind::InternetArchive,
                external_id: identifier.clone(),
                canonical_url: Some(format!("https://archive.org/details/{identifier}")),
                title,
                language: doc.get("language").and_then(json_first_string),
                document_type: DocumentType::BibliographicNotice,
                subject_links: vec![ExternalEntityRef {
                    system: "internet_archive".into(),
                    id: identifier,
                    label: Some(subject.label.clone()),
                }],
                publication_time: year.map(|y| crate::types::TypedTimeLite::Exact {
                    year: y,
                    surface: Some(y.to_string()),
                }),
                discovery_method: DiscoveryMethod::CatalogSearch,
                relevance_score: if authored { 0.8 } else { 0.52 },
                source_metadata: SourceMetadata {
                    raw: serde_json::json!({ "notice": text, "creator": creator, "place": place }),
                },
            });
        }
        out
    }
}

#[async_trait]
impl SourceConnector for InternetArchiveConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::InternetArchive
    }

    fn connector_version(&self) -> &str {
        "internet_archive:search_v1"
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
        let query = format!(
            "(creator:(\"{label}\") OR title:(\"{label}\")) AND mediatype:texts",
            label = subject.label
        );
        let rows = self.max_docs.to_string();
        let response = self
            .http
            .get(SEARCH)
            .query(&[
                ("q", query.as_str()),
                ("fl[]", "identifier"),
                ("fl[]", "title"),
                ("fl[]", "year"),
                ("fl[]", "creator"),
                ("fl[]", "description"),
                ("fl[]", "language"),
                ("output", "json"),
                ("rows", rows.as_str()),
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
            license: Some("Internet Archive metadata".into()),
            text,
        })
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        Ok(ConnectorHealth {
            ok: true,
            detail: "public advancedsearch.php".into(),
        })
    }
}
