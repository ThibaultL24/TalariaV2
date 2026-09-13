// crates/talaria-sources/src/connectors/pop_merimee.rs
//! POP/Mérimée connector for place entity and WGS84 coordinates.
//!
//! POP (Plateforme Ouverte du Patrimoine) includes Mérimée (historic monuments),
//! Palissy (movable heritage), and Mémoire (photographic archives).
//! This connector focuses on Mérimée for place-identity and WGS84 coordinates.
//!
//! Docs: https://pop.culture.gouv.fr/
//! API: https://api.pop.culture.gouv.fr/

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::kinds::{DiscoveryMethod, DocumentType, SourceKind};
use crate::plan::ResolvedSubject;
use crate::types::{DiscoveredDocument, SourceMetadata, TypedTimeLite};

pub const CONNECTOR_VERSION: &str = "pop_merimee:v1";
const API_BASE: &str = "https://api.pop.culture.gouv.fr";
const USER_AGENT: &str = "TalariaEngine/0.1 (+heritage; pop connector)";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerimeeRecord {
    pub ref_id: String,
    pub tico: Option<String>,           // Title
    pub dpro: Option<String>,           // Protection date
    pub adrs: Option<String>,           // Address
    pub com: Option<String>,            // Commune
    pub dept: Option<String>,           // Department
    pub reg: Option<String>,            // Region
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub hist: Option<String>,           // Historical description
    pub desc: Option<String>,           // Description
    pub siecle: Vec<String>,            // Centuries
    pub personnalite: Vec<String>,      // Related personalities
}

#[derive(Debug, Clone, Default)]
pub struct PopMerimeeConfig {
    pub timeout_secs: u64,
}

pub struct PopMerimeeConnector {
    http: reqwest::Client,
    config: PopMerimeeConfig,
}

impl PopMerimeeConnector {
    pub fn new(config: PopMerimeeConfig) -> anyhow::Result<Self> {
        let timeout = if config.timeout_secs > 0 {
            config.timeout_secs
        } else {
            30
        };
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(timeout))
            .build()?;
        Ok(Self { http, config })
    }

    pub async fn search_monuments(&self, query: &str) -> Result<Vec<MerimeeRecord>, ConnectorError> {
        let url = format!(
            "{API_BASE}/merimee/search?q={}&size=50",
            percent_encode(query)
        );

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;

        if resp.status().as_u16() == 429 {
            return Err(ConnectorError::RateLimited);
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ConnectorError::Http(format!("{status}: {}", truncate(&body, 200))));
        }

        let payload: Value = resp
            .json()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))?;

        Ok(parse_merimee_results(&payload))
    }

    pub async fn search_by_person(&self, person_name: &str) -> Result<Vec<MerimeeRecord>, ConnectorError> {
        let url = format!(
            "{API_BASE}/merimee/search?personnalite={}&size=30",
            percent_encode(person_name)
        );

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;

        if resp.status().as_u16() == 429 {
            return Err(ConnectorError::RateLimited);
        }

        if !resp.status().is_success() {
            // Fall back to general search if personality search fails
            return self.search_monuments(person_name).await;
        }

        let payload: Value = resp
            .json()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))?;

        Ok(parse_merimee_results(&payload))
    }

    pub async fn get_monument(&self, ref_id: &str) -> Result<Option<MerimeeRecord>, ConnectorError> {
        let url = format!("{API_BASE}/merimee/{ref_id}");

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;

        if resp.status().as_u16() == 404 {
            return Ok(None);
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ConnectorError::Http(format!("{status}: {}", truncate(&body, 200))));
        }

        let payload: Value = resp
            .json()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))?;

        Ok(parse_merimee_record(&payload))
    }
}

#[async_trait]
impl SourceConnector for PopMerimeeConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::PopMerimee
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

        let records = self.search_by_person(&subject.label).await?;
        let documents: Vec<DiscoveredDocument> = records
            .into_iter()
            .filter_map(|r| merimee_to_discovered(&r, subject))
            .collect();

        Ok(DiscoveryPage {
            documents,
            next_cursor: None,
        })
    }

    async fn fetch(
        &self,
        document: &DiscoveredDocument,
    ) -> Result<FetchedDocument, ConnectorError> {
        let ref_id = &document.external_id;
        let record = self.get_monument(ref_id).await?;

        let text = if let Some(r) = &record {
            build_merimee_text(r)
        } else {
            format!("Mérimée record not found: {ref_id}")
        };

        Ok(FetchedDocument {
            discovered: document.clone(),
            revision_id: None,
            content_type: "text/plain".into(),
            text,
            raw_metadata: record
                .map(|r| serde_json::to_value(r).unwrap_or_default())
                .unwrap_or_default(),
            license: Some("Licence Ouverte / Open Licence".into()),
            content_bytes: 0,
        })
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        let url = format!("{API_BASE}/merimee/search?q=Notre-Dame&size=1");
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;

        if resp.status().is_success() {
            Ok(ConnectorHealth {
                ok: true,
                detail: "POP Mérimée API reachable".into(),
            })
        } else {
            Ok(ConnectorHealth {
                ok: false,
                detail: format!("POP API HTTP {}", resp.status()),
            })
        }
    }
}

fn parse_merimee_results(payload: &Value) -> Vec<MerimeeRecord> {
    let hits = payload
        .get("hits")
        .or_else(|| payload.get("results"))
        .and_then(|v| v.as_array());

    let Some(hits) = hits else {
        return vec![];
    };

    hits.iter().filter_map(parse_merimee_record).collect()
}

fn parse_merimee_record(item: &Value) -> Option<MerimeeRecord> {
    let source = item.get("_source").unwrap_or(item);

    let ref_id = source
        .get("REF")
        .or_else(|| source.get("ref"))
        .and_then(|v| v.as_str())?
        .to_string();

    let tico = str_field(source, "TICO").or_else(|| str_field(source, "tico"));
    let dpro = str_field(source, "DPRO").or_else(|| str_field(source, "dpro"));
    let adrs = str_field(source, "ADRS").or_else(|| str_field(source, "adrs"));
    let com = str_field(source, "COM").or_else(|| str_field(source, "com"));
    let dept = str_field(source, "DPT").or_else(|| str_field(source, "dept"));
    let reg = str_field(source, "REG").or_else(|| str_field(source, "reg"));
    let hist = str_field(source, "HIST").or_else(|| str_field(source, "hist"));
    let desc = str_field(source, "DESC").or_else(|| str_field(source, "desc"));

    let lat = source
        .get("POP_COORDONNEES")
        .and_then(|c| c.get("lat"))
        .and_then(|v| v.as_f64())
        .or_else(|| source.get("lat").and_then(|v| v.as_f64()));

    let lon = source
        .get("POP_COORDONNEES")
        .and_then(|c| c.get("lon"))
        .and_then(|v| v.as_f64())
        .or_else(|| source.get("lon").and_then(|v| v.as_f64()));

    let siecle = str_array(source, "SCLE")
        .or_else(|| str_array(source, "siecle"))
        .unwrap_or_default();

    let personnalite = str_array(source, "PERS")
        .or_else(|| str_array(source, "personnalite"))
        .unwrap_or_default();

    Some(MerimeeRecord {
        ref_id,
        tico,
        dpro,
        adrs,
        com,
        dept,
        reg,
        lat,
        lon,
        hist,
        desc,
        siecle,
        personnalite,
    })
}

fn merimee_to_discovered(
    record: &MerimeeRecord,
    subject: &ResolvedSubject,
) -> Option<DiscoveredDocument> {
    let title = record.tico.as_ref()?;

    // Check if subject is mentioned in personalities or in title/description
    let subject_lower = subject.label.to_lowercase();
    let person_match = record.personnalite.iter().any(|p| {
        let p_lower = p.to_lowercase();
        p_lower.contains(&subject_lower) || subject_lower.contains(&p_lower)
    });

    let title_match = title.to_lowercase().contains(&subject_lower);
    let hist_match = record
        .hist
        .as_ref()
        .map(|h| h.to_lowercase().contains(&subject_lower))
        .unwrap_or(false);

    if !person_match && !title_match && !hist_match {
        return None;
    }

    let year = record
        .dpro
        .as_ref()
        .and_then(|d| d.get(..4))
        .and_then(|y| y.parse::<i32>().ok())
        .or_else(|| {
            record.siecle.first().and_then(|s| {
                // Parse century like "19e siècle" → 1850
                s.split(|c: char| !c.is_ascii_digit())
                    .find_map(|n| n.parse::<i32>().ok())
                    .map(|c| c * 100 - 50)
            })
        });

    let location = [
        record.adrs.as_deref(),
        record.com.as_deref(),
        record.dept.as_deref(),
    ]
    .iter()
    .filter_map(|&s| s)
    .collect::<Vec<_>>()
    .join(", ");

    Some(DiscoveredDocument {
        source_kind: SourceKind::PopMerimee,
        external_id: record.ref_id.clone(),
        canonical_url: Some(format!("https://www.pop.culture.gouv.fr/notice/merimee/{}", record.ref_id)),
        title: title.clone(),
        language: Some("fr".into()),
        document_type: DocumentType::Other("heritage_monument".into()),
        subject_links: vec![],
        publication_time: year.map(|y| TypedTimeLite::Exact {
            year: y,
            surface: record.dpro.clone(),
        }),
        discovery_method: DiscoveryMethod::CatalogSearch,
        relevance_score: if record.lat.is_some() && record.lon.is_some() {
            0.85
        } else {
            0.65
        },
        source_metadata: SourceMetadata {
            raw: serde_json::json!({
                "ref": record.ref_id,
                "location": location,
                "lat": record.lat,
                "lon": record.lon,
                "siecle": record.siecle,
                "personnalite": record.personnalite,
            }),
        },
    })
}

fn build_merimee_text(record: &MerimeeRecord) -> String {
    let mut parts = Vec::new();

    if let Some(title) = &record.tico {
        parts.push(format!("MONUMENT: {title}"));
    }

    let location = [
        record.adrs.as_deref(),
        record.com.as_deref(),
        record.dept.as_deref(),
        record.reg.as_deref(),
    ]
    .iter()
    .filter_map(|&s| s)
    .collect::<Vec<_>>()
    .join(", ");

    if !location.is_empty() {
        parts.push(format!("LOCATION: {location}"));
    }

    if let (Some(lat), Some(lon)) = (record.lat, record.lon) {
        parts.push(format!("COORDINATES: {lat:.6}, {lon:.6}"));
    }

    if !record.siecle.is_empty() {
        parts.push(format!("PERIOD: {}", record.siecle.join(", ")));
    }

    if let Some(hist) = &record.hist {
        parts.push(format!("HISTORY: {hist}"));
    }

    if !record.personnalite.is_empty() {
        parts.push(format!("PERSONALITIES: {}", record.personnalite.join(", ")));
    }

    parts.join("\n")
}

fn str_field(item: &Value, field: &str) -> Option<String> {
    item.get(field).and_then(|v| v.as_str()).map(str::to_string)
}

fn str_array(item: &Value, field: &str) -> Option<Vec<String>> {
    item.get(field).and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    })
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

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_results() {
        let payload = serde_json::json!({ "hits": [] });
        assert!(parse_merimee_results(&payload).is_empty());
    }

    #[test]
    fn test_parse_merimee_record() {
        let item = serde_json::json!({
            "REF": "PA00085991",
            "TICO": "Château de Malmaison",
            "COM": "Rueil-Malmaison",
            "DPT": "92",
            "REG": "Île-de-France",
            "POP_COORDONNEES": { "lat": 48.8701, "lon": 2.1657 },
            "SCLE": ["19e siècle"],
            "PERS": ["Napoléon Bonaparte", "Joséphine de Beauharnais"]
        });
        let result = parse_merimee_record(&item).unwrap();
        assert_eq!(result.ref_id, "PA00085991");
        assert_eq!(result.tico.as_deref(), Some("Château de Malmaison"));
        assert_eq!(result.lat, Some(48.8701));
        assert!(result.personnalite.contains(&"Napoléon Bonaparte".to_string()));
    }
}
