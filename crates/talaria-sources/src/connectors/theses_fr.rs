// crates/talaria-sources/src/connectors/theses_fr.rs
//! theses.fr (ABES) connector — discover/fetch with fixture or live HTTP.
//!
//! Search: GET /api/v1/theses/recherche/?q=&debut=&nombre=
//! Detail: GET /api/v1/theses/these/{id}

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
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

pub const CONNECTOR_VERSION: &str = "theses_fr:v1";
pub const DEFAULT_BASE_URL: &str = "https://theses.fr";
const USER_AGENT: &str = "TalariaEngine/0.1 (+corpus; theses_fr connector)";

#[derive(Debug, Clone)]
pub struct ThesesFrConfig {
    pub base_url: String,
    pub page_size: u32,
    pub fixture_dir: Option<PathBuf>,
    pub timeout: Duration,
}

impl Default for ThesesFrConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            page_size: 20,
            fixture_dir: None,
            timeout: Duration::from_secs(30),
        }
    }
}

pub struct ThesesFrConnector {
    config: ThesesFrConfig,
    client: Option<reqwest::Client>,
    /// external_id -> detail JSON (fixture mode)
    fixture_details: HashMap<String, serde_json::Value>,
    fixture_search: Option<serde_json::Value>,
}

impl ThesesFrConnector {
    pub fn new(config: ThesesFrConfig) -> Result<Self, ConnectorError> {
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
        let config = ThesesFrConfig {
            fixture_dir: Some(dir.as_ref().to_path_buf()),
            ..ThesesFrConfig::default()
        };
        Self::from_fixtures(config, dir.as_ref())
    }

    fn from_fixtures(config: ThesesFrConfig, dir: &Path) -> Result<Self, ConnectorError> {
        let search_path = dir.join("search.json");
        let search_raw = std::fs::read_to_string(&search_path)
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
                let id = external_id_from_detail(&value).ok_or_else(|| {
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

    fn build_query(subject: &ResolvedSubject) -> String {
        let mut escaped = subject.clone();
        escaped.label = escape_es_query(&subject.label);
        escaped.catalog_query(SourceKind::ThesesFr)
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
impl SourceConnector for ThesesFrConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::ThesesFr
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
        let nombre = self.config.page_size;

        let payload = if let Some(search) = &self.fixture_search {
            search.clone()
        } else {
            let q = Self::build_query(subject);
            let url = format!(
                "{}/api/v1/theses/recherche/?q={}&debut={debut}&nombre={nombre}&tri=pertinence",
                self.config.base_url.trim_end_matches('/'),
                urlencoding_encode(&q)
            );
            self.http_get_json(&url).await?
        };

        let theses = payload
            .get("theses")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let total = payload
            .get("totalHits")
            .and_then(|v| v.as_u64())
            .unwrap_or(theses.len() as u64);

        let mut documents = Vec::new();
        let slice: Vec<&serde_json::Value> = if self.fixture_search.is_some() {
            theses
                .iter()
                .skip(debut as usize)
                .take(nombre as usize)
                .collect()
        } else {
            theses.iter().take(nombre as usize).collect()
        };
        for item in slice {
            if let Some(doc) = lite_to_discovered(item) {
                documents.push(doc);
            }
        }

        let next_off = debut + documents.len() as u32;
        let total_cap = if self.fixture_search.is_some() {
            total.min(theses.len() as u64)
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
        } else if let Some(meta_id) = document
            .source_metadata
            .raw
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| self.fixture_details.contains_key(*s))
        {
            self.fixture_details.get(meta_id).unwrap().clone()
        } else {
            let id = document.external_id.clone();
            let url = format!(
                "{}/api/v1/theses/these/{}",
                self.config.base_url.trim_end_matches('/'),
                urlencoding_encode(&id)
            );
            self.http_get_json(&url).await?
        };

        let normalized = normalize_these_detail(&detail)?;
        Ok(FetchedDocument {
            discovered: document.clone(),
            revision_id: normalized.revision_token.clone(),
            content_type: "application/json".into(),
            text: normalized.snapshot_text.clone(),
            raw_metadata: json!({
                "normalized": normalized,
                "provider": detail,
            }),
            license: Some("Licence Ouverte 2.0".into()),
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
            "{}/api/v1/theses/recherche/?q=nnt:(000000000000)&nombre=1&debut=0",
            self.config.base_url.trim_end_matches('/')
        );
        match self.http_get_json(&url).await {
            Ok(_) => Ok(ConnectorHealth {
                ok: true,
                detail: "theses.fr search reachable".into(),
            }),
            Err(e) => Ok(ConnectorHealth {
                ok: false,
                detail: e.to_string(),
            }),
        }
    }
}

fn lite_to_discovered(item: &serde_json::Value) -> Option<DiscoveredDocument> {
    let external_id = external_id_from_lite(item)?;
    let title = item
        .get("titrePrincipal")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if title.is_empty() {
        return None;
    }
    let nnt = item.get("nnt").and_then(|v| v.as_str());
    let canonical_url = nnt
        .filter(|s| !s.is_empty())
        .map(|n| format!("https://theses.fr/{n}"));
    let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("");
    let document_type = if status.eq_ignore_ascii_case("soutenue")
        || item.get("nnt").and_then(|v| v.as_str()).is_some()
    {
        DocumentType::Thesis
    } else {
        DocumentType::BibliographicNotice
    };

    Some(DiscoveredDocument {
        source_kind: SourceKind::ThesesFr,
        external_id,
        canonical_url,
        title,
        language: Some("fr".into()),
        document_type,
        subject_links: vec![],
        publication_time: parse_year_field(
            item.get("dateSoutenance")
                .or_else(|| item.get("datePremiereInscriptionDoctorat")),
        ),
        discovery_method: DiscoveryMethod::CatalogSearch,
        relevance_score: 0.7,
        source_metadata: SourceMetadata { raw: item.clone() },
    })
}

pub fn normalize_these_detail(
    detail: &serde_json::Value,
) -> Result<NormalizedCorpusDocument, ConnectorError> {
    let external_id = external_id_from_detail(detail)
        .ok_or_else(|| ConnectorError::Parse("these detail missing nnt/numSujet/id".into()))?;

    let title = detail
        .get("titrePrincipal")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if title.is_empty() {
        return Err(ConnectorError::Parse("empty titrePrincipal".into()));
    }

    let status = detail
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let is_soutenue = detail
        .get("isSoutenue")
        .and_then(|v| v.as_bool())
        .unwrap_or(status == "soutenue");

    let academic_status = if is_soutenue || status == "soutenue" {
        AcademicStatus::DoctoralDefended
    } else if status == "encours" || status == "en_cours" {
        AcademicStatus::AcademicUnreviewed
    } else {
        AcademicStatus::CatalogRecord
    };

    let accessible = detail
        .get("accessible")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .eq_ignore_ascii_case("oui");
    // Independent axis: full-text availability is not derived from academic_status.
    let full_text_available = accessible;
    // Access level from rights/accessibility only — never from defended vs in-prep.
    let access_level = if accessible {
        AccessLevel::Open
    } else {
        AccessLevel::MetadataOnly
    };

    let abstracts = detail.get("resumes").cloned().unwrap_or(json!({}));
    let abstract_text = abstracts
        .get("fr")
        .or_else(|| abstracts.get("en"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let language = detail
        .get("langues")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let institution = detail
        .pointer("/etabSoutenance/nom")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let nnt = detail.get("nnt").and_then(|v| v.as_str());
    let canonical_url = nnt
        .filter(|s| !s.is_empty())
        .map(|n| format!("https://theses.fr/{n}"))
        .or_else(|| Some(format!("https://theses.fr/{external_id}")));

    let publication_time = parse_year_field(
        detail
            .get("dateSoutenance")
            .or_else(|| detail.get("datePremiereInscriptionDoctorat")),
    )
    .unwrap_or(TypedTimeLite::Unknown { surface: None });

    let mut identifiers = Vec::new();
    push_ident(&mut identifiers, IdentifierScheme::Nnt, nnt);
    push_ident(
        &mut identifiers,
        IdentifierScheme::Doi,
        detail.get("doi").and_then(|v| v.as_str()),
    );
    push_ident(
        &mut identifiers,
        IdentifierScheme::NumSujet,
        detail.get("numSujet").and_then(|v| v.as_str()),
    );
    // Establishment PPN is an institution id, not a document id — stored on contribution.

    let mut contributions = Vec::new();
    push_people(
        &mut contributions,
        ContributionRole::Author,
        detail.get("auteurs"),
    );
    push_people(
        &mut contributions,
        ContributionRole::ThesisAdvisor,
        detail.get("directeurs"),
    );
    push_people(
        &mut contributions,
        ContributionRole::JuryMember,
        detail.get("membresJury"),
    );
    push_people(
        &mut contributions,
        ContributionRole::Rapporteur,
        detail.get("rapporteurs"),
    );
    if let Some(pres) = detail.get("presidentJury") {
        push_person(&mut contributions, ContributionRole::JuryPresident, 0, pres);
    }
    if let Some(etab) = detail.get("etabSoutenance") {
        push_organism(&mut contributions, ContributionRole::Institution, 0, etab);
    }
    if let Some(arr) = detail.get("etabCotutelle").and_then(|v| v.as_array()) {
        for (i, o) in arr.iter().enumerate() {
            push_organism(
                &mut contributions,
                ContributionRole::CotutelleInstitution,
                i as i32,
                o,
            );
        }
    }
    if let Some(arr) = detail.get("ecolesDoctorales").and_then(|v| v.as_array()) {
        for (i, o) in arr.iter().enumerate() {
            push_organism(
                &mut contributions,
                ContributionRole::DoctoralSchool,
                i as i32,
                o,
            );
        }
    }
    if let Some(arr) = detail
        .get("partenairesRecherche")
        .and_then(|v| v.as_array())
    {
        for (i, o) in arr.iter().enumerate() {
            push_organism(
                &mut contributions,
                ContributionRole::ResearchPartner,
                i as i32,
                o,
            );
        }
    }

    let mut subjects = Vec::new();
    if let Some(map) = detail.get("mapSujets").and_then(|v| v.as_object()) {
        for (key, list) in map {
            let scheme = if key.to_ascii_lowercase().contains("rameau") {
                "rameau"
            } else {
                "keyword"
            };
            if let Some(arr) = list.as_array() {
                for item in arr {
                    let label = item
                        .get("keyword")
                        .or_else(|| item.get("libelle"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim();
                    if label.is_empty() {
                        continue;
                    }
                    let identifier = item
                        .get("ppn")
                        .or_else(|| item.get("query"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    subjects.push(NormalizedSubject {
                        scheme: scheme.into(),
                        label: label.into(),
                        identifier,
                    });
                }
            }
        }
    }
    // Also accept lite-style sujetsRameau on detail payloads.
    if let Some(arr) = detail.get("sujetsRameau").and_then(|v| v.as_array()) {
        for item in arr {
            let label = item
                .get("libelle")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if label.is_empty() {
                continue;
            }
            subjects.push(NormalizedSubject {
                scheme: "rameau".into(),
                label: label.into(),
                identifier: item
                    .get("ppn")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            });
        }
    }
    if let Some(arr) = detail.get("sujets").and_then(|v| v.as_array()) {
        for item in arr {
            let label = item
                .get("libelle")
                .or_else(|| item.get("keyword"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if label.is_empty() {
                continue;
            }
            subjects.push(NormalizedSubject {
                scheme: "keyword".into(),
                label: label.into(),
                identifier: None,
            });
        }
    }

    // document_type is independent of access/full_text:
    // in-prep → bibliographic notice; defended → thesis (even without full text).
    let document_type = if academic_status == AcademicStatus::AcademicUnreviewed {
        DocumentType::BibliographicNotice
    } else if academic_status == AcademicStatus::DoctoralDefended {
        DocumentType::Thesis
    } else {
        DocumentType::BibliographicNotice
    };

    let snapshot_text = build_snapshot_text(&title, abstract_text.as_deref(), &subjects);

    let mut doc = NormalizedCorpusDocument {
        source_kind: SourceKind::ThesesFr,
        external_id,
        canonical_url,
        document_type,
        title,
        language,
        abstract_text,
        academic_status,
        access_level,
        full_text_available,
        rights_uri: Some("https://www.etalab.gouv.fr/licence-ouverte-open-licence/".into()),
        rights_holder: Some("ABES / theses.fr".into()),
        rights_normalized: AccessLevel::Open,
        publisher_or_institution: institution,
        publication_time,
        identifiers,
        contributions,
        subjects,
        connector_version: CONNECTOR_VERSION.into(),
        snapshot_text,
        revision_token: None,
        raw_metadata: detail.clone(),
    };
    // Fingerprint covers all bibliographic axes so metadata edits create a new snapshot.
    let fp = doc.content_fingerprint();
    doc.revision_token = Some(fp);
    Ok(doc)
}

fn build_snapshot_text(
    title: &str,
    abstract_text: Option<&str>,
    subjects: &[NormalizedSubject],
) -> String {
    let mut parts = vec![format!("TITLE: {title}")];
    if let Some(a) = abstract_text {
        parts.push(format!("ABSTRACT: {a}"));
    }
    if !subjects.is_empty() {
        let joined = subjects
            .iter()
            .map(|s| format!("{}:{}", s.scheme, s.label))
            .collect::<Vec<_>>()
            .join(" | ");
        parts.push(format!("SUBJECTS: {joined}"));
    }
    parts.join("\n")
}

fn push_ident(out: &mut Vec<NormalizedIdentifier>, scheme: IdentifierScheme, raw: Option<&str>) {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return;
    };
    let Some(norm) = normalize_identifier(scheme, raw) else {
        return;
    };
    out.push(NormalizedIdentifier {
        scheme,
        value_raw: raw.to_string(),
        value_normalized: norm,
    });
}

fn push_people(
    out: &mut Vec<NormalizedContribution>,
    role: ContributionRole,
    value: Option<&serde_json::Value>,
) {
    let Some(arr) = value.and_then(|v| v.as_array()) else {
        return;
    };
    for (i, person) in arr.iter().enumerate() {
        push_person(out, role, i as i32, person);
    }
}

fn push_person(
    out: &mut Vec<NormalizedContribution>,
    role: ContributionRole,
    ordinal: i32,
    person: &serde_json::Value,
) {
    let nom = person
        .get("nom")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let prenom = person
        .get("prenom")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if nom.is_empty() && prenom.is_none() {
        return;
    }
    let agent_name = match prenom {
        Some(p) => format!("{p} {nom}").trim().to_string(),
        None => nom.to_string(),
    };
    let ppn = person
        .get("ppn")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| normalize_identifier(IdentifierScheme::Ppn, s));
    out.push(NormalizedContribution {
        role,
        agent_name: agent_name.clone(),
        name_normalized: normalize_person_name(nom, prenom),
        identifier_scheme: ppn.as_ref().map(|_| IdentifierScheme::Ppn),
        identifier_value: ppn,
        ordinal,
    });
}

fn push_organism(
    out: &mut Vec<NormalizedContribution>,
    role: ContributionRole,
    ordinal: i32,
    org: &serde_json::Value,
) {
    let nom = org.get("nom").and_then(|v| v.as_str()).unwrap_or("").trim();
    if nom.is_empty() {
        return;
    }
    let ppn = org
        .get("ppn")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| normalize_identifier(IdentifierScheme::Ppn, s));
    out.push(NormalizedContribution {
        role,
        agent_name: nom.to_string(),
        name_normalized: nom.to_ascii_lowercase(),
        identifier_scheme: ppn.as_ref().map(|_| IdentifierScheme::Ppn),
        identifier_value: ppn,
        ordinal,
    });
}

fn external_id_from_lite(item: &serde_json::Value) -> Option<String> {
    item.get("nnt")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            item.get("id")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        })
        .or_else(|| {
            item.get("numSujet")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        })
}

fn external_id_from_detail(detail: &serde_json::Value) -> Option<String> {
    external_id_from_lite(detail)
}

fn parse_year_field(value: Option<&serde_json::Value>) -> Option<TypedTimeLite> {
    let s = value.and_then(|v| v.as_str())?.trim();
    if s.is_empty() {
        return None;
    }
    let year: i32 = s.get(0..4)?.parse().ok()?;
    Some(TypedTimeLite::Exact {
        year,
        surface: Some(s.to_string()),
    })
}

fn escape_es_query(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '+' | '-' | '&' | '|' | '!' | '(' | ')' | '{' | '}' | '[' | ']' | '^' | '"' | '~'
            | '*' | '?' | ':' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
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

