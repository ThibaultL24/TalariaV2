// crates/talaria-sources/src/connectors/internet_archive.rs
//! Internet Archive advancedsearch + metadata — notices only, never file bytes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use super::net::{
    build_client, first_str, get_json, load_search_details, urlencoding_encode, year_from,
};
use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::corpus::{
    NormalizedContribution, NormalizedCorpusDocument, NormalizedIdentifier, NormalizedSubject,
};
use crate::identifiers::normalize_person_name;
use crate::kinds::{
    AcademicStatus, AccessLevel, ContributionRole, DiscoveryMethod, DocumentType, IdentifierScheme,
    SourceKind,
};
use crate::plan::ResolvedSubject;
use crate::types::{DiscoveredDocument, SourceMetadata, TypedTimeLite};

pub const CONNECTOR_VERSION: &str = "internet_archive:v1";
pub const DEFAULT_BASE_URL: &str = "https://archive.org";

#[derive(Debug, Clone)]
pub struct InternetArchiveConfig {
    pub base_url: String,
    pub page_size: u32,
    pub fixture_dir: Option<PathBuf>,
    pub timeout: Duration,
}

impl Default for InternetArchiveConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            page_size: 25,
            fixture_dir: None,
            timeout: Duration::from_secs(30),
        }
    }
}

pub struct InternetArchiveConnector {
    config: InternetArchiveConfig,
    client: Option<reqwest::Client>,
    fixture_details: HashMap<String, serde_json::Value>,
    fixture_search: Option<serde_json::Value>,
}

pub fn ia_id(v: &serde_json::Value) -> Option<String> {
    first_str(v, &["identifier"])
        .or_else(|| v.get("metadata").and_then(|m| first_str(m, &["identifier"])))
}

pub fn normalize_ia_item(raw: &serde_json::Value) -> Result<NormalizedCorpusDocument, ConnectorError> {
    let meta = raw.get("metadata").unwrap_or(raw);
    let external_id = ia_id(raw)
        .or_else(|| ia_id(meta))
        .ok_or_else(|| ConnectorError::Parse("internet archive missing identifier".into()))?;
    let title = first_str(meta, &["title"])
        .ok_or_else(|| ConnectorError::Parse("internet archive missing title".into()))?;
    let description = first_str(meta, &["description"]);
    let creator = first_str(meta, &["creator"]);
    let year_s = first_str(meta, &["year", "date"]);
    let language = first_str(meta, &["language"]);
    catalog_notice(
        SourceKind::InternetArchive,
        external_id.clone(),
        Some(format!("https://archive.org/details/{external_id}")),
        title,
        description,
        creator,
        year_s,
        language,
        first_str(meta, &["mediatype"]),
        None,
        CONNECTOR_VERSION,
        raw.clone(),
    )
}

impl InternetArchiveConnector {
    pub fn new(config: InternetArchiveConfig) -> Result<Self, ConnectorError> {
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
        let config = InternetArchiveConfig {
            fixture_dir: Some(dir.as_ref().to_path_buf()),
            ..InternetArchiveConfig::default()
        };
        Self::from_fixtures(config, dir.as_ref())
    }

    fn from_fixtures(config: InternetArchiveConfig, dir: &Path) -> Result<Self, ConnectorError> {
        let (search, details) = load_search_details(dir, ia_id)?;
        Ok(Self {
            config,
            client: None,
            fixture_details: details,
            fixture_search: Some(search),
        })
    }
}

#[async_trait]
impl SourceConnector for InternetArchiveConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::InternetArchive
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
            let q = subject.catalog_query(SourceKind::InternetArchive);
            let url = format!(
                "{}/advancedsearch.php?q={}&fl[]=identifier&fl[]=title&fl[]=creator&fl[]=year&fl[]=description&fl[]=mediatype&output=json&rows={nombre}&page={}",
                self.config.base_url.trim_end_matches('/'),
                urlencoding_encode(&q),
                (debut / nombre) + 1
            );
            get_json(client, &url).await?
        };
        let docs = payload
            .pointer("/response/docs")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let total = payload
            .pointer("/response/numFound")
            .and_then(|v| v.as_u64())
            .unwrap_or(docs.len() as u64);
        page_from_list(docs, debut, nombre, total, self.fixture_search.is_some(), lite_ia)
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
            let url = format!(
                "{}/metadata/{}",
                self.config.base_url.trim_end_matches('/'),
                urlencoding_encode(&document.external_id)
            );
            get_json(client, &url).await?
        };
        fetched(document, normalize_ia_item(&detail)?, detail)
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        Ok(ConnectorHealth {
            ok: true,
            detail: if self.fixture_search.is_some() {
                "fixture mode".into()
            } else {
                "internet archive metadata API".into()
            },
        })
    }
}

fn lite_ia(item: &serde_json::Value) -> Option<DiscoveredDocument> {
    let external_id = ia_id(item)?;
    let title = first_str(item, &["title"])?;
    Some(DiscoveredDocument {
        source_kind: SourceKind::InternetArchive,
        external_id: external_id.clone(),
        canonical_url: Some(format!("https://archive.org/details/{external_id}")),
        title,
        language: first_str(item, &["language"]),
        document_type: DocumentType::BibliographicNotice,
        subject_links: vec![],
        publication_time: first_str(item, &["year"]).and_then(|s| {
            year_from(&s).map(|y| TypedTimeLite::Exact {
                year: y,
                surface: Some(s),
            })
        }),
        discovery_method: DiscoveryMethod::CatalogSearch,
        relevance_score: 0.6,
        source_metadata: SourceMetadata { raw: item.clone() },
    })
}

pub(crate) fn catalog_notice(
    kind: SourceKind,
    external_id: String,
    canonical_url: Option<String>,
    title: String,
    description: Option<String>,
    creator: Option<String>,
    year_s: Option<String>,
    language: Option<String>,
    extra_subject: Option<String>,
    publisher: Option<String>,
    connector_version: &str,
    raw: serde_json::Value,
) -> Result<NormalizedCorpusDocument, ConnectorError> {
    let mut contributions = Vec::new();
    if let Some(name) = creator.filter(|s| !s.is_empty()) {
        contributions.push(NormalizedContribution {
            role: ContributionRole::Author,
            agent_name: name.clone(),
            name_normalized: normalize_person_name(&name, None),
            identifier_scheme: None,
            identifier_value: None,
            ordinal: 0,
        });
    }
    let mut subjects = Vec::new();
    if let Some(s) = extra_subject {
        subjects.push(NormalizedSubject {
            scheme: "mediatype".into(),
            label: s,
            identifier: None,
        });
    }
    let snapshot_text = match &description {
        Some(d) if !d.is_empty() => format!("{title}\n\n{d}"),
        _ => title.clone(),
    };
    let publication_time = match year_s.as_deref().and_then(year_from) {
        Some(y) => TypedTimeLite::Exact {
            year: y,
            surface: year_s,
        },
        None => TypedTimeLite::Unknown { surface: year_s },
    };
    let identifiers = vec![NormalizedIdentifier {
        scheme: IdentifierScheme::Other,
        value_raw: external_id.clone(),
        value_normalized: external_id.to_ascii_lowercase(),
    }];
    Ok(NormalizedCorpusDocument {
        source_kind: kind,
        external_id,
        canonical_url,
        document_type: DocumentType::BibliographicNotice,
        title,
        language,
        abstract_text: description,
        academic_status: AcademicStatus::CatalogRecord,
        access_level: AccessLevel::MetadataOnly,
        full_text_available: false,
        rights_uri: None,
        rights_holder: None,
        rights_normalized: AccessLevel::MetadataOnly,
        publisher_or_institution: publisher,
        publication_time,
        identifiers,
        contributions,
        subjects,
        connector_version: connector_version.into(),
        snapshot_text,
        revision_token: None,
        raw_metadata: raw,
    })
}

pub(crate) fn page_from_list(
    docs: Vec<serde_json::Value>,
    debut: u32,
    nombre: u32,
    total: u64,
    is_fixture: bool,
    lite: fn(&serde_json::Value) -> Option<DiscoveredDocument>,
) -> Result<DiscoveryPage, ConnectorError> {
    let slice: Vec<&serde_json::Value> = if is_fixture {
        docs.iter()
            .skip(debut as usize)
            .take(nombre as usize)
            .collect()
    } else {
        docs.iter().take(nombre as usize).collect()
    };
    let mut documents = Vec::new();
    for item in slice {
        if let Some(doc) = lite(item) {
            documents.push(doc);
        }
    }
    let next_off = debut + documents.len() as u32;
    let total_cap = if is_fixture {
        total.min(docs.len() as u64)
    } else {
        total
    };
    let next = if (next_off as u64) < total_cap && !documents.is_empty() {
        Some(DiscoveryCursor {
            token: None,
            offset: next_off,
        })
    } else {
        None
    };
    Ok(DiscoveryPage {
        documents,
        next_cursor: next,
    })
}

pub(crate) fn fetched(
    document: &DiscoveredDocument,
    normalized: NormalizedCorpusDocument,
    detail: serde_json::Value,
) -> Result<FetchedDocument, ConnectorError> {
    Ok(FetchedDocument {
        discovered: document.clone(),
        revision_id: normalized.revision_token.clone(),
        content_type: "application/json".into(),
        text: normalized.snapshot_text.clone(),
        raw_metadata: json!({
            "normalized": normalized,
            "provider": detail,
        }),
        license: None,
        content_bytes: normalized.snapshot_text.len() as u64,
    })
}
