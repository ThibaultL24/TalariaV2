// crates/talaria-sources/src/connectors/deutsche_biographie.rs
//! Deutsche Biographie + GND connector for structured life facts.
//!
//! Deutsche Biographie provides structured biographical data for German historical figures.
//! GND (Gemeinsame Normdatei) provides authority records with birth/death/occupation.
//!
//! Docs:
//! - https://www.deutsche-biographie.de/
//! - https://lobid.org/gnd (GND lookup via lobid.org)
//! API: lobid.org GND API (public, rate-limited)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::connectors::catalog::http_client;
use crate::kinds::{DiscoveryMethod, DocumentType, SourceKind};
use crate::plan::ResolvedSubject;
use crate::types::{DiscoveredDocument, SourceMetadata, TypedTimeLite};

pub const CONNECTOR_VERSION: &str = "deutsche_biographie:v1";
const GND_API_BASE: &str = "https://lobid.org/gnd";
const USER_AGENT: &str = "TalariaEngine/0.1 (+biography; db connector)";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GndPersonRecord {
    pub gnd_id: String,
    pub preferred_name: String,
    pub variant_names: Vec<String>,
    pub birth_date: Option<String>,
    pub birth_place: Option<String>,
    pub death_date: Option<String>,
    pub death_place: Option<String>,
    pub occupations: Vec<String>,
    pub profession_or_occupation: Vec<String>,
    pub biographical_information: Option<String>,
    pub wikipedia_link: Option<String>,
    pub deutsche_biographie_link: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DeutscheBiographieConfig {
    pub timeout_secs: u64,
}

pub struct DeutscheBiographieConnector {
    http: reqwest::Client,
    config: DeutscheBiographieConfig,
}

impl DeutscheBiographieConnector {
    pub fn new(config: DeutscheBiographieConfig) -> anyhow::Result<Self> {
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

    pub async fn search_person(&self, name: &str) -> Result<Vec<GndPersonRecord>, ConnectorError> {
        let url = format!(
            "{GND_API_BASE}/search?q={}&filter=type:Person&format=json&size=20",
            percent_encode(name)
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

        Ok(parse_gnd_results(&payload))
    }

    pub async fn get_person(&self, gnd_id: &str) -> Result<Option<GndPersonRecord>, ConnectorError> {
        let url = format!("{GND_API_BASE}/{gnd_id}.json");

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

        Ok(parse_gnd_person(&payload))
    }
}

#[async_trait]
impl SourceConnector for DeutscheBiographieConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::DeutscheBiographie
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

        let records = self.search_person(&subject.label).await?;
        let documents: Vec<DiscoveredDocument> = records
            .into_iter()
            .filter_map(|r| gnd_record_to_discovered(&r, subject))
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
        let gnd_id = &document.external_id;
        let record = self.get_person(gnd_id).await?;

        let text = if let Some(r) = &record {
            build_gnd_text(r)
        } else {
            format!("GND record not found: {gnd_id}")
        };

        Ok(FetchedDocument {
            discovered: document.clone(),
            revision_id: None,
            content_type: "text/plain".into(),
            text,
            raw_metadata: record
                .map(|r| serde_json::to_value(r).unwrap_or_default())
                .unwrap_or_default(),
            license: Some("CC0 (GND data)".into()),
            content_bytes: 0,
        })
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        let url = format!("{GND_API_BASE}/search?q=test&size=1&format=json");
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;

        if resp.status().is_success() {
            Ok(ConnectorHealth {
                ok: true,
                detail: "lobid.org GND API reachable".into(),
            })
        } else {
            Ok(ConnectorHealth {
                ok: false,
                detail: format!("GND API HTTP {}", resp.status()),
            })
        }
    }
}

fn parse_gnd_results(payload: &Value) -> Vec<GndPersonRecord> {
    let Some(members) = payload.get("member").and_then(|v| v.as_array()) else {
        return vec![];
    };

    members.iter().filter_map(parse_gnd_person).collect()
}

fn parse_gnd_person(item: &Value) -> Option<GndPersonRecord> {
    let gnd_id = item.get("gndIdentifier")?.as_str()?.to_string();
    let preferred_name = item.get("preferredName")?.as_str()?.to_string();

    let variant_names: Vec<String> = item
        .get("variantName")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();

    let birth_date = extract_date(item, "dateOfBirth");
    let birth_place = extract_place_label(item, "placeOfBirth");
    let death_date = extract_date(item, "dateOfDeath");
    let death_place = extract_place_label(item, "placeOfDeath");

    let occupations: Vec<String> = item
        .get("professionOrOccupation")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    v.get("label")
                        .and_then(|l| l.as_str())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();

    let profession_or_occupation: Vec<String> = item
        .get("professionOrOccupation")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.get("id").and_then(|i| i.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let biographical_information = item
        .get("biographicalOrHistoricalInformation")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let wikipedia_link = item
        .get("wikipedia")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|w| w.get("id"))
        .and_then(|i| i.as_str())
        .map(str::to_string);

    let deutsche_biographie_link = item
        .get("sameAs")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter()
                .filter_map(|s| s.get("id").and_then(|i| i.as_str()))
                .find(|s| s.contains("deutsche-biographie.de"))
                .map(str::to_string)
        });

    Some(GndPersonRecord {
        gnd_id,
        preferred_name,
        variant_names,
        birth_date,
        birth_place,
        death_date,
        death_place,
        occupations,
        profession_or_occupation,
        biographical_information,
        wikipedia_link,
        deutsche_biographie_link,
    })
}

fn extract_date(item: &Value, field: &str) -> Option<String> {
    item.get(field)
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

fn extract_place_label(item: &Value, field: &str) -> Option<String> {
    item.get(field)
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|p| p.get("label"))
        .and_then(|l| l.as_str())
        .map(str::to_string)
}

fn gnd_record_to_discovered(
    record: &GndPersonRecord,
    subject: &ResolvedSubject,
) -> Option<DiscoveredDocument> {
    // Basic name matching
    let record_lower = record.preferred_name.to_lowercase();
    let subject_lower = subject.label.to_lowercase();
    if !record_lower.contains(&subject_lower)
        && !subject_lower.contains(&record_lower)
        && !record.variant_names.iter().any(|v| {
            let v_lower = v.to_lowercase();
            v_lower.contains(&subject_lower) || subject_lower.contains(&v_lower)
        })
    {
        return None;
    }

    let year = record
        .birth_date
        .as_ref()
        .and_then(|d| d.get(..4))
        .and_then(|y| y.parse::<i32>().ok());

    Some(DiscoveredDocument {
        source_kind: SourceKind::DeutscheBiographie,
        external_id: record.gnd_id.clone(),
        canonical_url: Some(format!("https://d-nb.info/gnd/{}", record.gnd_id)),
        title: record.preferred_name.clone(),
        language: Some("de".into()),
        document_type: DocumentType::AuthorityRecord,
        subject_links: vec![],
        publication_time: year.map(|y| TypedTimeLite::Exact {
            year: y,
            surface: record.birth_date.clone(),
        }),
        discovery_method: DiscoveryMethod::CatalogSearch,
        relevance_score: 0.75,
        source_metadata: SourceMetadata {
            raw: serde_json::json!({
                "gnd_id": record.gnd_id,
                "birth_date": record.birth_date,
                "birth_place": record.birth_place,
                "death_date": record.death_date,
                "death_place": record.death_place,
                "occupations": record.occupations,
                "deutsche_biographie": record.deutsche_biographie_link,
            }),
        },
    })
}

fn build_gnd_text(record: &GndPersonRecord) -> String {
    let mut parts = vec![format!("NAME: {}", record.preferred_name)];

    if let Some(birth) = &record.birth_date {
        let place = record.birth_place.as_deref().unwrap_or("unknown");
        parts.push(format!("BORN: {birth} in {place}"));
    }

    if let Some(death) = &record.death_date {
        let place = record.death_place.as_deref().unwrap_or("unknown");
        parts.push(format!("DIED: {death} in {place}"));
    }

    if !record.occupations.is_empty() {
        parts.push(format!("OCCUPATIONS: {}", record.occupations.join(", ")));
    }

    if let Some(bio) = &record.biographical_information {
        parts.push(format!("BIO: {bio}"));
    }

    parts.join("\n")
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
        let payload = serde_json::json!({ "member": [] });
        assert!(parse_gnd_results(&payload).is_empty());
    }

    #[test]
    fn test_parse_gnd_person() {
        let item = serde_json::json!({
            "gndIdentifier": "118535935",
            "preferredName": "Friedrich II., Preußen, König",
            "variantName": ["Friedrich der Große", "Frederick the Great"],
            "dateOfBirth": ["1712-01-24"],
            "placeOfBirth": [{ "label": "Berlin" }],
            "dateOfDeath": ["1786-08-17"],
            "placeOfDeath": [{ "label": "Potsdam" }],
            "professionOrOccupation": [{ "label": "König", "id": "https://d-nb.info/gnd/4031516-2" }]
        });
        let result = parse_gnd_person(&item).unwrap();
        assert_eq!(result.gnd_id, "118535935");
        assert!(result.preferred_name.contains("Friedrich II"));
        assert_eq!(result.birth_date.as_deref(), Some("1712-01-24"));
        assert_eq!(result.birth_place.as_deref(), Some("Berlin"));
    }
}
