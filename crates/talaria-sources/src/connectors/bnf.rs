// crates/talaria-sources/src/connectors/bnf.rs
//! BnF catalogue SRU (Dublin Core) — bibliographic notices, never Gallica OCR bytes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;

use super::internet_archive::{catalog_notice, fetched, page_from_list};
use super::net::{
    build_client, first_str, get_text, load_search_details, urlencoding_encode, year_from,
};
use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::corpus::{NormalizedCorpusDocument, NormalizedIdentifier};
use crate::identifiers::normalize_identifier;
use crate::kinds::{DiscoveryMethod, DocumentType, IdentifierScheme, SourceKind};
use crate::plan::ResolvedSubject;
use crate::types::{DiscoveredDocument, SourceMetadata, TypedTimeLite};

pub const CONNECTOR_VERSION: &str = "bnf:v1";
pub const DEFAULT_BASE_URL: &str = "https://catalogue.bnf.fr/api/SRU";

#[derive(Debug, Clone)]
pub struct BnfConfig {
    pub base_url: String,
    pub page_size: u32,
    pub fixture_dir: Option<PathBuf>,
    pub timeout: Duration,
}

impl Default for BnfConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            page_size: 20,
            fixture_dir: None,
            timeout: Duration::from_secs(30),
        }
    }
}

pub struct BnfConnector {
    config: BnfConfig,
    client: Option<reqwest::Client>,
    fixture_details: HashMap<String, serde_json::Value>,
    fixture_search: Option<serde_json::Value>,
}

pub fn bnf_id(v: &serde_json::Value) -> Option<String> {
    first_str(v, &["ark", "external_id"]).map(|s| normalize_ark(&s))
}

fn normalize_ark(raw: &str) -> String {
    let t = raw.trim();
    if let Some(idx) = t.find("ark:") {
        t[idx..].trim_end_matches('/').to_string()
    } else {
        t.to_string()
    }
}

pub fn normalize_bnf_notice(
    raw: &serde_json::Value,
) -> Result<NormalizedCorpusDocument, ConnectorError> {
    let ark = bnf_id(raw).ok_or_else(|| ConnectorError::Parse("bnf missing ark".into()))?;
    let title = first_str(raw, &["title"])
        .ok_or_else(|| ConnectorError::Parse("bnf missing title".into()))?;
    let url = first_str(raw, &["canonical_url"])
        .or_else(|| Some(format!("https://catalogue.bnf.fr/{ark}")));
    let mut n = catalog_notice(
        SourceKind::Bnf,
        ark.clone(),
        url,
        title,
        first_str(raw, &["description"]),
        first_str(raw, &["creator"]),
        first_str(raw, &["date", "year"]),
        first_str(raw, &["language"]),
        None,
        first_str(raw, &["publisher"]),
        CONNECTOR_VERSION,
        raw.clone(),
    )?;
    if let Some(norm) = normalize_identifier(IdentifierScheme::Ark, &ark) {
        n.identifiers = vec![NormalizedIdentifier {
            scheme: IdentifierScheme::Ark,
            value_raw: ark,
            value_normalized: norm,
        }];
    }
    Ok(n)
}

impl BnfConnector {
    pub fn new(config: BnfConfig) -> Result<Self, ConnectorError> {
        if let Some(dir) = &config.fixture_dir {
            return Self::from_fixtures(config.clone(), dir);
        }
        Ok(Self {
            client: Some(build_client(config.timeout)?),
            config,
            fixture_details: HashMap::new(),
            fixture_search: None,
        })
    }

    pub fn from_fixture_dir(dir: impl AsRef<Path>) -> Result<Self, ConnectorError> {
        let config = BnfConfig {
            fixture_dir: Some(dir.as_ref().to_path_buf()),
            ..BnfConfig::default()
        };
        Self::from_fixtures(config, dir.as_ref())
    }

    fn from_fixtures(config: BnfConfig, dir: &Path) -> Result<Self, ConnectorError> {
        let (search, mut details) = load_search_details(dir, bnf_id)?;
        // Also key by ark suffix so fetch by either form works.
        let extra: Vec<(String, serde_json::Value)> = details
            .iter()
            .filter_map(|(k, v)| {
                k.rsplit('/').next().map(|s| (s.to_string(), v.clone()))
            })
            .collect();
        for (k, v) in extra {
            details.entry(k).or_insert(v);
        }
        Ok(Self {
            config,
            client: None,
            fixture_details: details,
            fixture_search: Some(search),
        })
    }
}

#[async_trait]
impl SourceConnector for BnfConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::Bnf
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
        let records = if let Some(search) = &self.fixture_search {
            search
                .get("records")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
        } else {
            let client = self.client.as_ref().ok_or_else(|| {
                ConnectorError::NotConfigured("live HTTP client missing".into())
            })?;
            let q = subject.catalog_query(SourceKind::Bnf);
            let url = format!(
                "{}?version=1.2&operation=searchRetrieve&recordSchema=dublincore&maximumRecords={nombre}&startRecord={}&query={}",
                self.config.base_url.trim_end_matches('/'),
                debut + 1,
                urlencoding_encode(&q)
            );
            let xml = get_text(client, &url).await?;
            parse_sru_dc(&xml)
        };
        let total = if let Some(search) = &self.fixture_search {
            search
                .get("total")
                .and_then(|v| v.as_u64())
                .unwrap_or(records.len() as u64)
        } else {
            records.len() as u64 + debut as u64
        };
        page_from_list(
            records,
            debut,
            nombre,
            total,
            self.fixture_search.is_some(),
            lite_bnf,
        )
    }

    async fn fetch(
        &self,
        document: &DiscoveredDocument,
    ) -> Result<FetchedDocument, ConnectorError> {
        let detail = self
            .fixture_details
            .get(&document.external_id)
            .cloned()
            .or_else(|| {
                document
                    .external_id
                    .rsplit('/')
                    .next()
                    .and_then(|s| self.fixture_details.get(s).cloned())
            })
            .or_else(|| {
                if document.source_metadata.raw.get("title").is_some() {
                    Some(document.source_metadata.raw.clone())
                } else {
                    None
                }
            });
        let detail = if let Some(d) = detail {
            d
        } else {
            let client = self.client.as_ref().ok_or_else(|| {
                ConnectorError::NotConfigured("live HTTP client missing".into())
            })?;
            let q = format!("bib.persistentid all \"{}\"", document.external_id);
            let url = format!(
                "{}?version=1.2&operation=searchRetrieve&recordSchema=dublincore&maximumRecords=1&query={}",
                self.config.base_url.trim_end_matches('/'),
                urlencoding_encode(&q)
            );
            let xml = get_text(client, &url).await?;
            parse_sru_dc(&xml)
                .into_iter()
                .next()
                .ok_or_else(|| ConnectorError::Parse("bnf SRU empty".into()))?
        };
        fetched(document, normalize_bnf_notice(&detail)?, detail)
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        Ok(ConnectorHealth {
            ok: true,
            detail: if self.fixture_search.is_some() {
                "fixture mode".into()
            } else {
                "bnf catalogue SRU".into()
            },
        })
    }
}

fn lite_bnf(item: &serde_json::Value) -> Option<DiscoveredDocument> {
    let external_id = bnf_id(item)?;
    let title = first_str(item, &["title"])?;
    Some(DiscoveredDocument {
        source_kind: SourceKind::Bnf,
        external_id: external_id.clone(),
        canonical_url: first_str(item, &["canonical_url"])
            .or_else(|| Some(format!("https://catalogue.bnf.fr/{external_id}"))),
        title,
        language: first_str(item, &["language"]),
        document_type: DocumentType::BibliographicNotice,
        subject_links: vec![],
        publication_time: first_str(item, &["date", "year"]).and_then(|s| {
            year_from(&s).map(|y| TypedTimeLite::Exact {
                year: y,
                surface: Some(s),
            })
        }),
        discovery_method: DiscoveryMethod::CatalogSearch,
        relevance_score: 0.65,
        source_metadata: SourceMetadata { raw: item.clone() },
    })
}

fn xml_dc(xml: &str, tag: &str) -> Option<String> {
    for ns in ["dc:", ""] {
        let open = format!("<{ns}{tag}>");
        let close = format!("</{ns}{tag}>");
        if let Some(a) = xml.find(&open) {
            let start = a + open.len();
            if let Some(rel) = xml[start..].find(&close) {
                let text = xml[start..start + rel]
                    .replace("&amp;", "&")
                    .replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&quot;", "\"")
                    .trim()
                    .to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

fn parse_sru_dc(xml: &str) -> Vec<serde_json::Value> {
    let mut records = Vec::new();
    let mut rest = xml;
    while let Some(idx) = rest.find("<srw:record").or_else(|| rest.find("<record")) {
        rest = &rest[idx + 1..];
        let end = rest
            .find("</srw:record>")
            .or_else(|| rest.find("</record>"))
            .unwrap_or(rest.len());
        let block = &rest[..end];
        if let Some(title) = xml_dc(block, "title") {
            let ident = xml_dc(block, "identifier").unwrap_or_default();
            let ark = if ident.contains("ark:") {
                normalize_ark(&ident)
            } else {
                xml_dc(block, "identifier")
                    .map(|s| normalize_ark(&s))
                    .unwrap_or_default()
            };
            if ark.is_empty() {
                rest = &rest[end..];
                continue;
            }
            records.push(serde_json::json!({
                "ark": ark,
                "title": title,
                "description": xml_dc(block, "description"),
                "date": xml_dc(block, "date"),
                "creator": xml_dc(block, "creator"),
                "language": xml_dc(block, "language"),
                "publisher": xml_dc(block, "publisher"),
                "canonical_url": ident,
            }));
        }
        rest = &rest[end..];
    }
    records
}
