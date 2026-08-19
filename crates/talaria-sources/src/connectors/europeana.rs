// crates/talaria-sources/src/connectors/europeana.rs
//! Europeana Search API v2 — metadata notices, never media bytes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;

use super::internet_archive::{catalog_notice, fetched, page_from_list};
use super::net::{
    build_client, first_str, get_json, load_search_details, urlencoding_encode,
};
use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::corpus::NormalizedCorpusDocument;
use crate::kinds::{DiscoveryMethod, DocumentType, SourceKind};
use crate::plan::ResolvedSubject;
use crate::types::{DiscoveredDocument, SourceMetadata, TypedTimeLite};

pub const CONNECTOR_VERSION: &str = "europeana:v1";
pub const DEFAULT_BASE_URL: &str = "https://api.europeana.eu/record/v2";

#[derive(Debug, Clone)]
pub struct EuropeanaConfig {
    pub base_url: String,
    pub page_size: u32,
    pub fixture_dir: Option<PathBuf>,
    pub timeout: Duration,
    pub api_key: Option<String>,
}

impl Default for EuropeanaConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            page_size: 25,
            fixture_dir: None,
            timeout: Duration::from_secs(30),
            api_key: None,
        }
    }
}

pub struct EuropeanaConnector {
    config: EuropeanaConfig,
    client: Option<reqwest::Client>,
    fixture_details: HashMap<String, serde_json::Value>,
    fixture_search: Option<serde_json::Value>,
}

pub fn europeana_id(v: &serde_json::Value) -> Option<String> {
    first_str(v, &["id"]).or_else(|| v.get("object").and_then(|o| first_str(o, &["id"])))
}

pub fn normalize_europeana_item(
    raw: &serde_json::Value,
) -> Result<NormalizedCorpusDocument, ConnectorError> {
    let item = raw.get("object").unwrap_or(raw);
    let external_id = first_str(item, &["id"])
        .ok_or_else(|| ConnectorError::Parse("europeana missing id".into()))?;
    let title = first_str(item, &["title", "dcTitle"])
        .ok_or_else(|| ConnectorError::Parse("europeana missing title".into()))?;
    let description = first_str(item, &["dcDescription", "description"]);
    let provider = first_str(item, &["dataProvider", "provider"]);
    let url = first_str(item, &["edmIsShownAt", "guid", "edmIsShownBy"])
        .or_else(|| Some(format!("https://www.europeana.eu/item{external_id}")));
    catalog_notice(
        SourceKind::Europeana,
        external_id,
        url,
        title,
        description,
        first_str(item, &["dcCreator", "creator"]),
        first_str(item, &["year"]),
        first_str(item, &["language", "dcLanguage"]),
        first_str(item, &["type"]),
        provider,
        CONNECTOR_VERSION,
        raw.clone(),
    )
}

impl EuropeanaConnector {
    pub fn new(config: EuropeanaConfig) -> Result<Self, ConnectorError> {
        if let Some(dir) = &config.fixture_dir {
            return Self::from_fixtures(config.clone(), dir);
        }
        if config
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_none()
        {
            return Err(ConnectorError::NotConfigured(
                "EUROPEANA_API_KEY required for live Europeana".into(),
            ));
        }
        Ok(Self {
            client: Some(build_client(config.timeout)?),
            config,
            fixture_details: HashMap::new(),
            fixture_search: None,
        })
    }

    pub fn from_fixture_dir(dir: impl AsRef<Path>) -> Result<Self, ConnectorError> {
        let config = EuropeanaConfig {
            fixture_dir: Some(dir.as_ref().to_path_buf()),
            ..EuropeanaConfig::default()
        };
        Self::from_fixtures(config, dir.as_ref())
    }

    fn from_fixtures(config: EuropeanaConfig, dir: &Path) -> Result<Self, ConnectorError> {
        let (search, details) = load_search_details(dir, europeana_id)?;
        Ok(Self {
            config,
            client: None,
            fixture_details: details,
            fixture_search: Some(search),
        })
    }
}

#[async_trait]
impl SourceConnector for EuropeanaConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::Europeana
    }
    fn connector_version(&self) -> &str {
        CONNECTOR_VERSION
    }

    async fn discover(
        &self,
        subject: &ResolvedSubject,
        cursor: Option<DiscoveryCursor>,
    ) -> Result<DiscoveryPage, ConnectorError> {
        let debut = cursor.map(|c| c.offset).unwrap_or(0);
        let nombre = self.config.page_size.max(1);
        let payload = if let Some(search) = &self.fixture_search {
            search.clone()
        } else {
            let client = self.client.as_ref().ok_or_else(|| {
                ConnectorError::NotConfigured("live HTTP client missing".into())
            })?;
            let key = self
                .config
                .api_key
                .as_deref()
                .ok_or_else(|| ConnectorError::NotConfigured("EUROPEANA_API_KEY".into()))?;
            let q = subject.catalog_query(SourceKind::Europeana);
            let url = format!(
                "{}/search.json?wskey={}&query={}&qf=TYPE:TEXT&rows={nombre}&start={}",
                self.config.base_url.trim_end_matches('/'),
                urlencoding_encode(key),
                urlencoding_encode(&q),
                debut + 1
            );
            get_json(client, &url).await?
        };
        let docs = payload
            .get("items")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let total = payload
            .get("totalResults")
            .and_then(|v| v.as_u64())
            .unwrap_or(docs.len() as u64);
        page_from_list(
            docs,
            debut,
            nombre,
            total,
            self.fixture_search.is_some(),
            lite_eu,
        )
    }

    async fn fetch(
        &self,
        document: &DiscoveredDocument,
    ) -> Result<FetchedDocument, ConnectorError> {
        let detail = if let Some(v) = self.fixture_details.get(&document.external_id) {
            v.clone()
        } else {
            let client = self.client.as_ref().ok_or_else(|| {
                ConnectorError::NotConfigured("live HTTP client missing".into())
            })?;
            let key = self
                .config
                .api_key
                .as_deref()
                .ok_or_else(|| ConnectorError::NotConfigured("EUROPEANA_API_KEY".into()))?;
            let id = document.external_id.trim_start_matches('/');
            let url = format!(
                "{}/{}.json?wskey={}",
                self.config.base_url.trim_end_matches('/'),
                id,
                urlencoding_encode(key)
            );
            get_json(client, &url).await?
        };
        fetched(document, normalize_europeana_item(&detail)?, detail)
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        Ok(ConnectorHealth {
            ok: true,
            detail: if self.fixture_search.is_some() {
                "fixture mode".into()
            } else {
                "europeana search API".into()
            },
        })
    }
}

fn lite_eu(item: &serde_json::Value) -> Option<DiscoveredDocument> {
    let external_id = europeana_id(item)?;
    let title = first_str(item, &["title"])?;
    Some(DiscoveredDocument {
        source_kind: SourceKind::Europeana,
        external_id,
        canonical_url: first_str(item, &["edmIsShownAt", "guid"]),
        title,
        language: first_str(item, &["language"]),
        document_type: DocumentType::BibliographicNotice,
        subject_links: vec![],
        publication_time: first_str(item, &["year"]).and_then(|s| {
            s.parse::<i32>().ok().map(|y| TypedTimeLite::Exact {
                year: y,
                surface: Some(s),
            })
        }),
        discovery_method: DiscoveryMethod::CatalogSearch,
        relevance_score: 0.6,
        source_metadata: SourceMetadata { raw: item.clone() },
    })
}
