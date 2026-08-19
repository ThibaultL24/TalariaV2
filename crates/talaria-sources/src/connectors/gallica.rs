// crates/talaria-sources/src/connectors/gallica.rs
use async_trait::async_trait;

use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::connectors::catalog::{
    bibliographic_notice, catalog_place, http_client, names_match, parse_year, xml_texts,
    year_in_life, NoticeRelation,
};
use crate::kinds::{DiscoveryMethod, DocumentType, SourceKind};
use crate::plan::ResolvedSubject;
use crate::types::{DiscoveredDocument, ExternalEntityRef, SourceMetadata};

const SRU: &str = "https://gallica.bnf.fr/SRU";

pub struct GallicaConnector {
    http: reqwest::Client,
    max_docs: u32,
}

impl GallicaConnector {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            http: http_client()?,
            max_docs: 15,
        })
    }

    pub fn parse_sru(subject: &ResolvedSubject, xml: &str) -> Vec<DiscoveredDocument> {
        let chunks: Vec<&str> = if xml.contains("<srw:record>") {
            xml.split("<srw:record>").skip(1).collect()
        } else {
            xml.split("<record>").skip(1).collect()
        };
        let mut out = Vec::new();
        for chunk in chunks {
            let title = xml_texts(chunk, "dc:title")
                .into_iter()
                .next()
                .unwrap_or_else(|| "Sans titre".into());
            let identifier = xml_texts(chunk, "dc:identifier")
                .into_iter()
                .find(|id| id.contains("gallica.bnf.fr") || id.contains("ark:"))
                .or_else(|| xml_texts(chunk, "dc:identifier").into_iter().next())
                .unwrap_or_default();
            if identifier.is_empty() {
                continue;
            }
            let year = xml_texts(chunk, "dc:date")
                .iter()
                .find_map(|d| parse_year(d));
            let creators = xml_texts(chunk, "dc:creator");
            let authored = creators
                .iter()
                .any(|name| names_match(&subject.label, name));
            if authored {
                if let Some(year) = year {
                    if !year_in_life(year, subject) {
                        continue;
                    }
                }
            }
            let place = catalog_place(
                xml_texts(chunk, "dc:coverage")
                    .into_iter()
                    .next()
                    .or_else(|| xml_texts(chunk, "dc:publisher").into_iter().next())
                    .as_deref(),
            );
            let description = xml_texts(chunk, "dc:description").into_iter().next();
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
                source_kind: SourceKind::Gallica,
                external_id: identifier.clone(),
                canonical_url: Some(identifier.clone()),
                title,
                language: xml_texts(chunk, "dc:language").into_iter().next(),
                document_type: DocumentType::BibliographicNotice,
                subject_links: vec![ExternalEntityRef {
                    system: "gallica".into(),
                    id: identifier,
                    label: Some(subject.label.clone()),
                }],
                publication_time: year.map(|y| crate::types::TypedTimeLite::Exact {
                    year: y,
                    surface: Some(y.to_string()),
                }),
                discovery_method: DiscoveryMethod::CatalogSearch,
                relevance_score: if authored { 0.8 } else { 0.54 },
                source_metadata: SourceMetadata {
                    raw: serde_json::json!({ "notice": text, "creators": creators }),
                },
            });
        }
        out
    }
}

#[async_trait]
impl SourceConnector for GallicaConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::Gallica
    }

    fn connector_version(&self) -> &str {
        "gallica:sru_v1"
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
        let query = format!("gallica all \"{}\"", subject.label);
        let max = self.max_docs.to_string();
        let xml = self
            .http
            .get(SRU)
            .query(&[
                ("version", "1.2"),
                ("operation", "searchRetrieve"),
                ("query", query.as_str()),
                ("maximumRecords", max.as_str()),
            ])
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .error_for_status()
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .text()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;
        Ok(DiscoveryPage {
            documents: Self::parse_sru(subject, &xml),
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
            license: Some("BnF / Gallica terms".into()),
            text,
        })
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        Ok(ConnectorHealth {
            ok: true,
            detail: "public SRU".into(),
        })
    }
}
