// crates/talaria-sources/src/connectors/wikisource.rs
use async_trait::async_trait;
use serde_json::Value;

use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::connectors::catalog::http_client;
use crate::kinds::{DiscoveryMethod, DocumentType, SourceKind};
use crate::plan::ResolvedSubject;
use crate::types::{DiscoveredDocument, ExternalEntityRef, SourceMetadata};

const EXTRACT_CHARS: &str = "8000";
const DEFAULT_LANGS: &[&str] = &["en", "fr"];

#[derive(Debug, Clone)]
pub struct WikisourceConnectorConfig {
    pub languages: Vec<String>,
    pub max_pages: u32,
}

impl Default for WikisourceConnectorConfig {
    fn default() -> Self {
        Self {
            languages: DEFAULT_LANGS.iter().map(|s| (*s).to_string()).collect(),
            max_pages: 8,
        }
    }
}

pub struct WikisourceConnector {
    http: reqwest::Client,
    config: WikisourceConnectorConfig,
}

impl WikisourceConnector {
    pub fn new(config: WikisourceConnectorConfig) -> anyhow::Result<Self> {
        Ok(Self {
            http: http_client()?,
            config,
        })
    }

    fn api(lang: &str) -> String {
        format!("https://{lang}.wikisource.org/w/api.php")
    }

    fn languages_for(subject: &ResolvedSubject, configured: &[String]) -> Vec<String> {
        let mut langs: Vec<String> = subject
            .languages
            .iter()
            .map(|l| l.chars().take(2).collect::<String>().to_lowercase())
            .filter(|l| l.len() == 2 && l.chars().all(|c| c.is_ascii_alphabetic()))
            .collect();
        if langs.is_empty() {
            langs = configured.to_vec();
        }
        langs.sort();
        langs.dedup();
        langs.truncate(3);
        langs
    }

    pub fn parse_search_titles(payload: &Value) -> Vec<(String, i64)> {
        payload
            .pointer("/query/search")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|hit| {
                let title = hit.get("title").and_then(|t| t.as_str())?;
                if title.is_empty() {
                    return None;
                }
                let ns = hit.get("ns").and_then(|n| n.as_i64()).unwrap_or(0);
                Some((title.to_string(), ns))
            })
            .collect()
    }

    pub fn parse_extract(payload: &Value) -> Result<(String, Value), ConnectorError> {
        let pages = payload
            .pointer("/query/pages")
            .and_then(|v| v.as_object())
            .ok_or_else(|| ConnectorError::Parse("no pages".into()))?;
        let page = pages
            .values()
            .next()
            .ok_or_else(|| ConnectorError::Parse("empty pages".into()))?;
        if page.get("missing").is_some() {
            return Err(ConnectorError::Parse("missing page".into()));
        }
        let extract = page
            .get("extract")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok((extract, page.clone()))
    }

    fn document_type(title: &str, ns: i64) -> DocumentType {
        let lower = title.to_ascii_lowercase();
        if ns != 0 || lower.starts_with("author:") || lower.starts_with("auteur:") {
            DocumentType::Correspondence
        } else {
            DocumentType::BookOcr
        }
    }

    fn discovered(
        lang: &str,
        title: &str,
        ns: i64,
        method: DiscoveryMethod,
        score: f32,
    ) -> DiscoveredDocument {
        DiscoveredDocument {
            source_kind: SourceKind::Wikisource,
            external_id: format!("{lang}:{title}"),
            canonical_url: Some(format!(
                "https://{lang}.wikisource.org/wiki/{}",
                title.replace(' ', "_")
            )),
            title: title.to_string(),
            language: Some(lang.to_string()),
            document_type: Self::document_type(title, ns),
            subject_links: vec![ExternalEntityRef {
                system: "wikisource".into(),
                id: format!("{lang}:{title}"),
                label: Some(title.to_string()),
            }],
            publication_time: None,
            discovery_method: method,
            relevance_score: score,
            source_metadata: SourceMetadata {
                raw: serde_json::json!({ "lang": lang, "ns": ns }),
            },
        }
    }

    async fn search_titles(
        &self,
        lang: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(String, i64)>, ConnectorError> {
        if limit == 0 || query.trim().is_empty() {
            return Ok(vec![]);
        }
        let response = self
            .http
            .get(Self::api(lang))
            .query(&[
                ("action", "query"),
                ("list", "search"),
                ("srsearch", query),
                ("srlimit", &limit.min(20).to_string()),
                ("srnamespace", "0|102|106"),
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
        Ok(Self::parse_search_titles(&response))
    }

    async fn fetch_extract(
        &self,
        lang: &str,
        title: &str,
    ) -> Result<(String, Value), ConnectorError> {
        let response = self
            .http
            .get(Self::api(lang))
            .query(&[
                ("action", "query"),
                ("prop", "extracts|info"),
                ("explaintext", "1"),
                ("exlimit", "1"),
                ("exchars", EXTRACT_CHARS),
                ("titles", title),
                ("format", "json"),
                ("redirects", "1"),
            ])
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .error_for_status()
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .json::<Value>()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))?;
        Self::parse_extract(&response)
    }
}

#[async_trait]
impl SourceConnector for WikisourceConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::Wikisource
    }

    fn connector_version(&self) -> &str {
        "wikisource:extracts_v1"
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
        let search_q = subject.catalog_query(SourceKind::Wikisource);
        let langs = Self::languages_for(subject, &self.config.languages);
        let mut docs = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for lang in &langs {
            let hits = self
                .search_titles(lang, &search_q, self.config.max_pages as usize)
                .await
                .unwrap_or_default();
            for (title, ns) in hits {
                if !seen.insert(format!("{lang}:{title}")) {
                    continue;
                }
                let score = if ns != 0 { 0.9 } else { 0.72 };
                docs.push(Self::discovered(
                    lang,
                    &title,
                    ns,
                    DiscoveryMethod::CatalogSearch,
                    score,
                ));
            }
        }
        Ok(DiscoveryPage {
            documents: docs,
            next_cursor: None,
        })
    }

    async fn fetch(
        &self,
        document: &DiscoveredDocument,
    ) -> Result<FetchedDocument, ConnectorError> {
        let lang = document.language.as_deref().unwrap_or("en");
        let (text, page) = self.fetch_extract(lang, &document.title).await?;
        if text.trim().is_empty() {
            return Err(ConnectorError::Parse(format!(
                "empty extract {}",
                document.title
            )));
        }
        let revid = page
            .get("lastrevid")
            .and_then(|v| v.as_u64())
            .map(|n| n.to_string());
        let bytes = text.len() as u64;
        Ok(FetchedDocument {
            discovered: document.clone(),
            revision_id: revid,
            content_type: "text/plain".into(),
            text,
            raw_metadata: page,
            license: Some("CC BY-SA".into()),
            content_bytes: bytes,
        })
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        Ok(ConnectorHealth {
            ok: true,
            detail: format!("langs={:?}", self.config.languages),
        })
    }
}
