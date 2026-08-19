// crates/talaria-sources/src/connectors/hal.rs
//! HAL open archive connector (Solr REST API).
//!
//! Docs: https://api.hal.science/docs/search
//! Endpoint: https://api.hal.science/search/?q=…&fl=…&wt=json&rows=N&start=N

use async_trait::async_trait;
use serde_json::Value;

use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::connectors::catalog::{http_client, names_match, year_in_life};
use crate::corpus::{
    NormalizedContribution, NormalizedCorpusDocument, NormalizedIdentifier, NormalizedSubject,
};
use crate::identifiers::normalize_identifier;
use crate::kinds::{
    AcademicStatus, AccessLevel, ContributionRole, DiscoveryMethod, DocumentType, IdentifierScheme,
    SourceKind,
};
use crate::plan::ResolvedSubject;
use crate::types::{DiscoveredDocument, SourceMetadata, TypedTimeLite};

pub const CONNECTOR_VERSION: &str = "hal:v1";
const BASE_URL: &str = "https://api.hal.science/search/";
const USER_AGENT: &str = "TalariaEngine/0.1 (+corpus; hal connector)";
const DEFAULT_ROWS: u32 = 20;

/// Fields requested from the Solr API (keep minimal to reduce payload).
const FIELDS: &str =
    "halId_s,title_s,abstract_s,producedDate_tdate,authFullName_s,keyword_s,docType_s,\
     uri_s,doi_s,openAccess_bool,language_s";

pub struct HalConnector {
    http: reqwest::Client,
    rows: u32,
}

impl HalConnector {
    pub fn new() -> anyhow::Result<Self> {
        let http = http_client()?;
        Ok(Self {
            http,
            rows: DEFAULT_ROWS,
        })
    }

    async fn search(
        &self,
        query: &str,
        start: u32,
    ) -> Result<Value, ConnectorError> {
        let url = format!(
            "{BASE_URL}?q={}&fl={}&wt=json&rows={}&start={}",
            percent_encode(query),
            FIELDS,
            self.rows,
            start
        );

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ConnectorError::Http(format!("{status}: {body}")));
        }

        resp.json()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))
    }
}

#[async_trait]
impl SourceConnector for HalConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::Hal
    }

    fn connector_version(&self) -> &str {
        CONNECTOR_VERSION
    }

    async fn discover(
        &self,
        subject: &ResolvedSubject,
        cursor: Option<DiscoveryCursor>,
    ) -> Result<DiscoveryPage, ConnectorError> {
        let start = cursor.map(|c| c.offset).unwrap_or(0);
        if start > 0 {
            return Ok(DiscoveryPage {
                documents: vec![],
                next_cursor: None,
            });
        }

        let queries = subject.catalog_query_buckets(SourceKind::Hal);
        let mut documents = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for query in queries {
            let payload = self.search(&query, 0).await?;
            let response = payload.get("response").ok_or_else(|| {
                ConnectorError::Parse("HAL response missing 'response' key".into())
            })?;
            let docs = response
                .get("docs")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            for doc in &docs {
                if let Some(d) = hal_doc_to_discovered(doc, subject) {
                    if seen.insert(d.external_id.clone()) {
                        documents.push(d);
                    }
                }
            }
        }

        Ok(DiscoveryPage {
            documents,
            next_cursor: None,
        })
    }

    async fn fetch(
        &self,
        document: &DiscoveredDocument,
    ) -> Result<FetchedDocument, ConnectorError> {
        let hal_id = &document.external_id;
        let url = format!(
            "{BASE_URL}?q=halId_s:{}&fl={}&wt=json&rows=1",
            percent_encode(hal_id),
            FIELDS
        );

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ConnectorError::Http(format!("{status}: {body}")));
        }

        let payload: Value = resp
            .json()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))?;

        let raw = payload
            .pointer("/response/docs/0")
            .cloned()
            .ok_or_else(|| ConnectorError::Parse(format!("HAL record not found: {hal_id}")))?;

        let normalized = normalize_hal_doc(&raw)?;
        let text = normalized.snapshot_text.clone();
        let revision = normalized.revision_token.clone();

        Ok(FetchedDocument {
            discovered: document.clone(),
            revision_id: revision,
            content_type: "application/json".into(),
            text,
            raw_metadata: serde_json::json!({ "normalized": normalized, "provider": raw }),
            license: Some("open access (CC)".into()),
            content_bytes: 0,
        })
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        let url = format!("{BASE_URL}?q=*:*&wt=json&rows=0");
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;
        if resp.status().is_success() {
            Ok(ConnectorHealth {
                ok: true,
                detail: "HAL API reachable".into(),
            })
        } else {
            Ok(ConnectorHealth {
                ok: false,
                detail: format!("HTTP {}", resp.status()),
            })
        }
    }
}

fn hal_doc_to_discovered(
    doc: &Value,
    subject: &ResolvedSubject,
) -> Option<DiscoveredDocument> {
    let hal_id = doc.get("halId_s")?.as_str()?.trim().to_string();
    if hal_id.is_empty() {
        return None;
    }

    let title = first_str_field(doc, "title_s")?;
    if title.is_empty() {
        return None;
    }

    // Accept documents that either mention the subject by name in authors or title/abstract.
    let authors = str_array(doc, "authFullName_s");
    let author_match = authors.iter().any(|a| names_match(&subject.label, a));
    if !author_match {
        // Also accept: subject name appears in title or abstract.
        let title_lower = title.to_lowercase();
        let label_lower = subject.label.to_lowercase();
        if !title_lower.contains(&label_lower) {
            let abstract_hit = doc
                .get("abstract_s")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .any(|a| {
                    a.as_str()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&label_lower)
                });
            if !abstract_hit {
                return None;
            }
        }
    }

    let year = doc
        .get("producedDate_tdate")
        .and_then(|v| v.as_str())
        .and_then(|s| s.get(..4))
        .and_then(|y| y.parse::<i32>().ok());

    if author_match {
        if let Some(y) = year {
            if !year_in_life(y, subject) {
                return None;
            }
        }
    }

    let doc_type = hal_doc_type(doc.get("docType_s").and_then(|v| v.as_str()).unwrap_or(""));

    let uri = doc
        .get("uri_s")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Some(DiscoveredDocument {
        source_kind: SourceKind::Hal,
        external_id: hal_id,
        canonical_url: uri,
        title,
        language: doc.get("language_s").and_then(|v| v.as_str()).map(Into::into),
        document_type: doc_type,
        subject_links: vec![],
        publication_time: year.map(|y| TypedTimeLite::Exact {
            year: y,
            surface: Some(y.to_string()),
        }),
        discovery_method: DiscoveryMethod::CatalogSearch,
        relevance_score: 0.75,
        source_metadata: SourceMetadata { raw: doc.clone() },
    })
}

pub fn normalize_hal_doc(doc: &Value) -> Result<NormalizedCorpusDocument, ConnectorError> {
    let hal_id = doc
        .get("halId_s")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ConnectorError::Parse("HAL doc missing halId_s".into()))?
        .to_string();

    let title = first_str_field(doc, "title_s")
        .ok_or_else(|| ConnectorError::Parse("HAL doc missing title".into()))?;

    let abstract_text = doc
        .get("abstract_s")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let year = doc
        .get("producedDate_tdate")
        .and_then(|v| v.as_str())
        .and_then(|s| s.get(..4))
        .and_then(|y| y.parse::<i32>().ok());

    let publication_time = year
        .map(|y| TypedTimeLite::Exact {
            year: y,
            surface: Some(y.to_string()),
        })
        .unwrap_or(TypedTimeLite::Unknown { surface: None });

    let language = doc
        .get("language_s")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let open_access = doc
        .get("openAccess_bool")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let access_level = if open_access {
        AccessLevel::Open
    } else {
        AccessLevel::MetadataOnly
    };

    let uri = doc
        .get("uri_s")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let doi = doc
        .get("doi_s")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .or_else(|| doc.get("doi_s").and_then(|v| v.as_str()))
        .map(|s| s.to_string());

    let mut identifiers = Vec::new();
    identifiers.push(NormalizedIdentifier {
        scheme: IdentifierScheme::HalId,
        value_raw: hal_id.clone(),
        value_normalized: hal_id.clone(),
    });
    if let Some(d) = doi.as_deref() {
        if let Some(norm) = normalize_identifier(IdentifierScheme::Doi, d) {
            identifiers.push(NormalizedIdentifier {
                scheme: IdentifierScheme::Doi,
                value_raw: d.to_string(),
                value_normalized: norm,
            });
        }
    }

    let contributions: Vec<NormalizedContribution> = str_array(doc, "authFullName_s")
        .into_iter()
        .enumerate()
        .map(|(i, name)| NormalizedContribution {
            role: ContributionRole::Author,
            agent_name: name.clone(),
            name_normalized: name.to_lowercase(),
            identifier_scheme: None,
            identifier_value: None,
            ordinal: i as i32,
        })
        .collect();

    let subjects: Vec<NormalizedSubject> = str_array(doc, "keyword_s")
        .into_iter()
        .map(|kw| NormalizedSubject {
            scheme: "keyword".into(),
            label: kw,
            identifier: None,
        })
        .collect();

    let doc_type = hal_doc_type(doc.get("docType_s").and_then(|v| v.as_str()).unwrap_or(""));

    let snapshot_text = build_hal_text(&title, abstract_text.as_deref(), &subjects);

    let mut normalized = NormalizedCorpusDocument {
        source_kind: SourceKind::Hal,
        external_id: hal_id,
        canonical_url: uri,
        document_type: doc_type,
        title,
        language,
        abstract_text,
        academic_status: AcademicStatus::PeerReviewed,
        access_level,
        full_text_available: open_access,
        rights_uri: Some("https://hal.science".into()),
        rights_holder: Some("HAL / CCSD".into()),
        rights_normalized: access_level,
        publisher_or_institution: None,
        publication_time,
        identifiers,
        contributions,
        subjects,
        connector_version: CONNECTOR_VERSION.into(),
        snapshot_text,
        revision_token: None,
        raw_metadata: doc.clone(),
    };
    let fp = normalized.content_fingerprint();
    normalized.revision_token = Some(fp);
    Ok(normalized)
}

fn build_hal_text(title: &str, abstract_text: Option<&str>, subjects: &[NormalizedSubject]) -> String {
    let mut parts = vec![format!("TITLE: {title}")];
    if let Some(a) = abstract_text {
        parts.push(format!("ABSTRACT: {a}"));
    }
    if !subjects.is_empty() {
        let joined = subjects
            .iter()
            .map(|s| s.label.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        parts.push(format!("KEYWORDS: {joined}"));
    }
    parts.join("\n")
}

fn hal_doc_type(doc_type: &str) -> DocumentType {
    match doc_type.to_uppercase().as_str() {
        "THESE" | "HDR" => DocumentType::Thesis,
        "ART" | "COMM" | "POSTER" | "PRESCONF" | "PROCEEDINGS" => DocumentType::AcademicArticle,
        "BOOK" | "COUV" | "DOUV" | "CHAPTER" => DocumentType::BibliographicNotice,
        _ => DocumentType::AcademicArticle,
    }
}

fn first_str_field(doc: &Value, field: &str) -> Option<String> {
    if let Some(arr) = doc.get(field).and_then(|v| v.as_array()) {
        return arr
            .first()
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
    }
    doc.get(field)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn str_array(doc: &Value, field: &str) -> Vec<String> {
    doc.get(field)
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
