// crates/talaria-sources/src/connectors/openalex.rs
//! OpenAlex works connector — title + abstract only, never PDF bytes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::corpus::{
    NormalizedContribution, NormalizedCorpusDocument, NormalizedIdentifier, NormalizedSubject,
};
use crate::identifiers::{normalize_identifier, normalize_person_name};
use crate::kinds::{
    AcademicStatus, AccessLevel, ContributionRole, DiscoveryMethod, DocumentType, IdentifierScheme,
    SourceKind,
};
use crate::plan::ResolvedSubject;
use crate::types::{DiscoveredDocument, SourceMetadata, TypedTimeLite};

pub const CONNECTOR_VERSION: &str = "openalex:v1";
pub const DEFAULT_BASE_URL: &str = "https://api.openalex.org";
const USER_AGENT: &str = "TalariaEngine/0.1 (+corpus; openalex connector)";

const DEBATE_TERMS: &str =
    "(origins OR historiography OR controversy OR origines OR nationality OR birthplace OR identity)";

#[derive(Debug, Clone)]
pub struct OpenAlexConfig {
    pub base_url: String,
    pub page_size: u32,
    pub fixture_dir: Option<PathBuf>,
    pub timeout: Duration,
    pub mailto: Option<String>,
}

impl Default for OpenAlexConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            page_size: 25,
            fixture_dir: None,
            timeout: Duration::from_secs(30),
            mailto: None,
        }
    }
}

pub struct OpenAlexConnector {
    config: OpenAlexConfig,
    client: Option<reqwest::Client>,
    fixture_details: HashMap<String, serde_json::Value>,
    fixture_search: Option<serde_json::Value>,
}

pub fn openalex_debate_query(label: &str) -> String {
    let label = label.trim();
    format!("\"{label}\" {DEBATE_TERMS}")
}

pub fn reconstruct_abstract(index: &serde_json::Value) -> Option<String> {
    let obj = index.as_object()?;
    let mut slots: Vec<(u64, &str)> = Vec::new();
    for (word, positions) in obj {
        let Some(arr) = positions.as_array() else {
            continue;
        };
        for p in arr {
            if let Some(i) = p.as_u64() {
                slots.push((i, word.as_str()));
            }
        }
    }
    if slots.is_empty() {
        return None;
    }
    slots.sort_by_key(|(i, _)| *i);
    Some(
        slots
            .into_iter()
            .map(|(_, w)| w)
            .collect::<Vec<_>>()
            .join(" "),
    )
}

pub fn normalize_openalex_work(
    work: &serde_json::Value,
) -> Result<NormalizedCorpusDocument, ConnectorError> {
    let external_id = work_id(work)
        .ok_or_else(|| ConnectorError::Parse("openalex work missing id".into()))?;
    let title = work
        .get("display_name")
        .or_else(|| work.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if title.is_empty() {
        return Err(ConnectorError::Parse("openalex work missing title".into()));
    }

    let abstract_text = work
        .get("abstract_inverted_index")
        .and_then(reconstruct_abstract);

    let work_type = work
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("article")
        .to_ascii_lowercase();
    let (document_type, academic_status) = match work_type.as_str() {
        "dissertation" => (DocumentType::Thesis, AcademicStatus::DoctoralDefended),
        "preprint" => (DocumentType::AcademicArticle, AcademicStatus::AcademicUnreviewed),
        "book" | "book-chapter" => (DocumentType::BibliographicNotice, AcademicStatus::PeerReviewed),
        _ => (DocumentType::AcademicArticle, AcademicStatus::PeerReviewed),
    };

    let oa_status = work
        .get("open_access")
        .and_then(|o| o.get("oa_status"))
        .and_then(|v| v.as_str())
        .unwrap_or("closed")
        .to_ascii_lowercase();
    let access_level = match oa_status.as_str() {
        "gold" | "hybrid" | "bronze" | "green" => AccessLevel::Open,
        _ => AccessLevel::MetadataOnly,
    };

    let year = work
        .get("publication_year")
        .and_then(|v| v.as_i64())
        .map(|y| y as i32);
    let publication_time = match year {
        Some(y) => TypedTimeLite::Exact {
            year: y,
            surface: Some(y.to_string()),
        },
        None => TypedTimeLite::Unknown { surface: None },
    };

    let mut identifiers = Vec::new();
    if let Some(raw) = work.get("doi").and_then(|v| v.as_str()) {
        if let Some(norm) = normalize_identifier(IdentifierScheme::Doi, raw) {
            identifiers.push(NormalizedIdentifier {
                scheme: IdentifierScheme::Doi,
                value_raw: raw.into(),
                value_normalized: norm,
            });
        }
    }

    let mut contributions = Vec::new();
    if let Some(auths) = work.get("authorships").and_then(|v| v.as_array()) {
        for (i, a) in auths.iter().enumerate() {
            let name = a
                .get("author")
                .and_then(|au| au.get("display_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if name.is_empty() {
                continue;
            }
            contributions.push(NormalizedContribution {
                role: ContributionRole::Author,
                agent_name: name.into(),
                name_normalized: normalize_person_name(name, None),
                identifier_scheme: None,
                identifier_value: a
                    .get("author")
                    .and_then(|au| au.get("orcid"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                ordinal: i as i32,
            });
        }
    }

    let mut subjects = Vec::new();
    if let Some(topics) = work.get("topics").and_then(|v| v.as_array()) {
        for t in topics {
            if let Some(label) = t.get("display_name").and_then(|v| v.as_str()) {
                subjects.push(NormalizedSubject {
                    scheme: "openalex_topic".into(),
                    label: label.into(),
                    identifier: t.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                });
            }
        }
    }

    let landing = work
        .get("primary_location")
        .and_then(|l| l.get("landing_page_url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let canonical_url = work
        .get("doi")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or(landing)
        .or_else(|| Some(format!("https://openalex.org/{external_id}")));

    let publisher = work
        .get("primary_location")
        .and_then(|l| l.get("source"))
        .and_then(|s| s.get("display_name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let language = work
        .get("language")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let snapshot_text = match &abstract_text {
        Some(a) if !a.is_empty() => format!("{title}\n\n{a}"),
        _ => title.clone(),
    };

    Ok(NormalizedCorpusDocument {
        source_kind: SourceKind::OpenAlex,
        external_id,
        canonical_url,
        document_type,
        title,
        language,
        abstract_text,
        academic_status,
        access_level,
        full_text_available: false,
        rights_uri: Some("https://openalex.org".into()),
        rights_holder: Some("OpenAlex".into()),
        rights_normalized: AccessLevel::Open,
        publisher_or_institution: publisher,
        publication_time,
        identifiers,
        contributions,
        subjects,
        connector_version: CONNECTOR_VERSION.into(),
        snapshot_text,
        revision_token: work
            .get("updated_date")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        raw_metadata: work.clone(),
    })
}

impl OpenAlexConnector {
    pub fn new(config: OpenAlexConfig) -> Result<Self, ConnectorError> {
        if let Some(dir) = &config.fixture_dir {
            return Self::from_fixtures(config.clone(), dir);
        }
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(config.timeout)
            .build()
            .map_err(|e| ConnectorError::Http(e.to_string()))?;
        Ok(Self {
            config,
            client: Some(client),
            fixture_details: HashMap::new(),
            fixture_search: None,
        })
    }

    pub fn from_fixture_dir(dir: impl AsRef<Path>) -> Result<Self, ConnectorError> {
        let config = OpenAlexConfig {
            fixture_dir: Some(dir.as_ref().to_path_buf()),
            ..OpenAlexConfig::default()
        };
        Self::from_fixtures(config, dir.as_ref())
    }

    fn from_fixtures(config: OpenAlexConfig, dir: &Path) -> Result<Self, ConnectorError> {
        let search_raw = std::fs::read_to_string(dir.join("search.json"))
            .map_err(|e| ConnectorError::Other(anyhow::anyhow!("fixture search: {e}")))?;
        let search: serde_json::Value = serde_json::from_str(&search_raw)
            .map_err(|e| ConnectorError::Parse(format!("fixture search: {e}")))?;

        let mut details = HashMap::new();
        let details_dir = dir.join("details");
        if details_dir.is_dir() {
            for entry in std::fs::read_dir(&details_dir)
                .map_err(|e| ConnectorError::Other(anyhow::anyhow!("{e}")))?
            {
                let entry = entry.map_err(|e| ConnectorError::Other(anyhow::anyhow!("{e}")))?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let raw = std::fs::read_to_string(&path)
                    .map_err(|e| ConnectorError::Other(anyhow::anyhow!("{e}")))?;
                let value: serde_json::Value =
                    serde_json::from_str(&raw).map_err(|e| ConnectorError::Parse(e.to_string()))?;
                let id = work_id(&value).ok_or_else(|| {
                    ConnectorError::Parse(format!("missing id in {}", path.display()))
                })?;
                details.insert(id, value);
            }
        }

        Ok(Self {
            config,
            client: None,
            fixture_details: details,
            fixture_search: Some(search),
        })
    }

    async fn http_get_json(&self, url: &str) -> Result<serde_json::Value, ConnectorError> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| ConnectorError::NotConfigured("live HTTP client missing".into()))?;
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let resp = client
                .get(url)
                .send()
                .await
                .map_err(|e| ConnectorError::Http(e.to_string()))?;
            let status = resp.status();
            if status.as_u16() == 429 {
                if attempt >= 3 {
                    return Err(ConnectorError::RateLimited);
                }
                let wait = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(2u64.saturating_pow(attempt));
                tokio::time::sleep(Duration::from_secs(wait.min(30))).await;
                continue;
            }
            if status.is_server_error() && attempt < 3 {
                tokio::time::sleep(Duration::from_millis(200 * attempt as u64)).await;
                continue;
            }
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(ConnectorError::Http(format!("{status}: {body}")));
            }
            return resp
                .json()
                .await
                .map_err(|e| ConnectorError::Parse(e.to_string()));
        }
    }
}

#[async_trait]
impl SourceConnector for OpenAlexConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::OpenAlex
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
            let q = subject.catalog_query(SourceKind::OpenAlex);
            let page = (debut / nombre) + 1;
            let mut url = format!(
                "{}/works?search={}&per-page={nombre}&page={page}",
                self.config.base_url.trim_end_matches('/'),
                urlencoding_encode(&q)
            );
            if let Some(mail) = &self.config.mailto {
                url.push_str("&mailto=");
                url.push_str(&urlencoding_encode(mail));
            }
            self.http_get_json(&url).await?
        };

        let results = payload
            .get("results")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let total = payload
            .get("meta")
            .and_then(|m| m.get("count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(results.len() as u64);

        let slice: Vec<&serde_json::Value> = if self.fixture_search.is_some() {
            results
                .iter()
                .skip(debut as usize)
                .take(nombre as usize)
                .collect()
        } else {
            results.iter().take(nombre as usize).collect()
        };

        let mut documents = Vec::new();
        for item in slice {
            if let Some(doc) = lite_to_discovered(item) {
                documents.push(doc);
            }
        }

        let next_off = debut + documents.len() as u32;
        let total_cap = if self.fixture_search.is_some() {
            total.min(results.len() as u64)
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

    async fn fetch(
        &self,
        document: &DiscoveredDocument,
    ) -> Result<FetchedDocument, ConnectorError> {
        let detail = if let Some(v) = self.fixture_details.get(&document.external_id) {
            v.clone()
        } else {
            let mut url = format!(
                "{}/works/{}",
                self.config.base_url.trim_end_matches('/'),
                urlencoding_encode(&document.external_id)
            );
            if let Some(mail) = &self.config.mailto {
                url.push_str("?mailto=");
                url.push_str(&urlencoding_encode(mail));
            }
            self.http_get_json(&url).await?
        };

        let normalized = normalize_openalex_work(&detail)?;
        Ok(FetchedDocument {
            discovered: document.clone(),
            revision_id: normalized.revision_token.clone(),
            content_type: "application/json".into(),
            text: normalized.snapshot_text.clone(),
            raw_metadata: json!({
                "normalized": normalized,
                "provider": detail,
            }),
            license: Some("OpenAlex metadata".into()),
            content_bytes: normalized.snapshot_text.len() as u64,
        })
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        if self.fixture_search.is_some() {
            return Ok(ConnectorHealth {
                ok: true,
                detail: format!("fixture mode ({} details)", self.fixture_details.len()),
            });
        }
        let url = format!(
            "{}/works?filter=openalex_id:W2741809807&per-page=1",
            self.config.base_url.trim_end_matches('/')
        );
        match self.http_get_json(&url).await {
            Ok(_) => Ok(ConnectorHealth {
                ok: true,
                detail: "openalex works reachable".into(),
            }),
            Err(e) => Ok(ConnectorHealth {
                ok: false,
                detail: e.to_string(),
            }),
        }
    }
}

fn work_id(work: &serde_json::Value) -> Option<String> {
    let id = work.get("id").and_then(|v| v.as_str())?;
    let tail = id.rsplit('/').next().unwrap_or(id).trim();
    if tail.is_empty() {
        None
    } else {
        Some(tail.to_ascii_uppercase())
    }
}

fn lite_to_discovered(item: &serde_json::Value) -> Option<DiscoveredDocument> {
    let external_id = work_id(item)?;
    let title = item
        .get("display_name")
        .or_else(|| item.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if title.is_empty() {
        return None;
    }
    let year = item
        .get("publication_year")
        .and_then(|v| v.as_i64())
        .map(|y| y as i32);
    Some(DiscoveredDocument {
        source_kind: SourceKind::OpenAlex,
        external_id,
        canonical_url: item
            .get("doi")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                Some(format!(
                    "https://openalex.org/{}",
                    work_id(item).unwrap_or_default()
                ))
            }),
        title,
        language: item
            .get("language")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        document_type: DocumentType::AcademicArticle,
        subject_links: vec![],
        publication_time: year.map(|y| TypedTimeLite::Exact {
            year: y,
            surface: Some(y.to_string()),
        }),
        discovery_method: DiscoveryMethod::CatalogSearch,
        relevance_score: 0.65,
        source_metadata: SourceMetadata { raw: item.clone() },
    })
}

fn urlencoding_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
