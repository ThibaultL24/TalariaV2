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
use crate::corpus::NormalizedCorpusDocument;
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

        let queries = subject.catalog_query_buckets(SourceKind::Gallica);
        let mut documents = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let per_query = (self.max_docs / queries.len().max(1) as u32).max(5);
        let max = per_query.to_string();

        for query in queries {
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

            for doc in Self::parse_sru(subject, &xml) {
                if seen.insert(doc.external_id.clone()) {
                    documents.push(doc);
                }
            }
        }

        documents.truncate(self.max_docs as usize);
        Ok(DiscoveryPage {
            documents,
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
        let normalized = normalize_gallica_notice(document, &text)?;
        Ok(FetchedDocument {
            discovered: document.clone(),
            revision_id: normalized.revision_token.clone(),
            content_type: "text/plain".into(),
            content_bytes: text.len() as u64,
            raw_metadata: serde_json::json!({ "normalized": normalized, "notice": text }),
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

pub fn normalize_gallica_notice(
    document: &DiscoveredDocument,
    notice_text: &str,
) -> Result<NormalizedCorpusDocument, ConnectorError> {
    use crate::corpus::NormalizedIdentifier;
    use crate::kinds::{AcademicStatus, AccessLevel, IdentifierScheme};
    use crate::types::TypedTimeLite;

    let year = document.publication_time.as_ref().and_then(|t| match t {
        TypedTimeLite::Exact { year, .. } => Some(*year),
        _ => None,
    });

    let publication_time = year
        .map(|y| TypedTimeLite::Exact {
            year: y,
            surface: Some(y.to_string()),
        })
        .unwrap_or(TypedTimeLite::Unknown { surface: None });

    let mut doc = NormalizedCorpusDocument {
        source_kind: SourceKind::Gallica,
        external_id: document.external_id.clone(),
        canonical_url: document.canonical_url.clone(),
        document_type: DocumentType::BibliographicNotice,
        title: document.title.clone(),
        language: document.language.clone(),
        abstract_text: Some(notice_text.to_string()),
        academic_status: AcademicStatus::PrimarySource,
        access_level: AccessLevel::Open,
        full_text_available: true,
        rights_uri: Some("https://gallica.bnf.fr/html/und/conditions-dutilisation".into()),
        rights_holder: Some("BnF / Gallica".into()),
        rights_normalized: AccessLevel::Open,
        publisher_or_institution: None,
        publication_time,
        identifiers: vec![NormalizedIdentifier {
            scheme: IdentifierScheme::Ark,
            value_raw: document.external_id.clone(),
            value_normalized: document.external_id.clone(),
        }],
        contributions: vec![],
        subjects: vec![],
        connector_version: "gallica:sru_v1".into(),
        snapshot_text: notice_text.to_string(),
        revision_token: None,
        raw_metadata: document.source_metadata.raw.clone(),
    };
    let fp = doc.content_fingerprint();
    doc.revision_token = Some(fp);
    Ok(doc)
}
