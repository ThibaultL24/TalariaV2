// crates/talaria-sources/src/connectors/whg.rs
//! World Historical Gazetteer (WHG) connector.
//!
//! WHG provides place-identity grounding with historical place attestations.
//! API requires authentication token (WHG_API_TOKEN env var).
//!
//! Docs: https://whgazetteer.org/api/
//! API base: https://whgazetteer.org/api/

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::kinds::SourceKind;
use crate::plan::ResolvedSubject;
use crate::types::DiscoveredDocument;

pub const CONNECTOR_VERSION: &str = "whg:v1";
const API_BASE: &str = "https://whgazetteer.org/api";
const USER_AGENT: &str = "TalariaEngine/0.1 (+grounding; whg connector)";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhgPlaceMatch {
    pub whg_id: String,
    pub title: String,
    pub ccodes: Vec<String>,
    pub types: Vec<String>,
    pub geom: Option<WhgGeometry>,
    pub when: Option<WhgTemporal>,
    pub links: Vec<WhgLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhgGeometry {
    pub r#type: String,
    pub coordinates: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhgTemporal {
    pub timespans: Vec<WhgTimespan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhgTimespan {
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhgLink {
    pub r#type: String,
    pub identifier: String,
}

#[derive(Debug, Clone, Default)]
pub struct WhgConfig {
    pub api_token: Option<String>,
    pub timeout_secs: u64,
}

impl WhgConfig {
    pub fn from_env() -> Self {
        Self {
            api_token: std::env::var("WHG_API_TOKEN").ok().filter(|s| !s.is_empty()),
            timeout_secs: 30,
        }
    }
}

pub struct WhgConnector {
    http: reqwest::Client,
    config: WhgConfig,
}

impl WhgConnector {
    pub fn new(config: WhgConfig) -> anyhow::Result<Self> {
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

    pub fn is_configured(&self) -> bool {
        self.config.api_token.is_some()
    }

    pub async fn search_places(&self, query: &str) -> Result<Vec<WhgPlaceMatch>, ConnectorError> {
        let token = self.config.api_token.as_ref().ok_or_else(|| {
            ConnectorError::NotConfigured("WHG_API_TOKEN environment variable not set".into())
        })?;

        let url = format!("{API_BASE}/places/?q={}", percent_encode(query));
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Token {token}"))
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;

        if resp.status().as_u16() == 401 {
            return Err(ConnectorError::NotConfigured(
                "WHG API token invalid or expired".into(),
            ));
        }

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

        Ok(parse_whg_results(&payload))
    }

    pub async fn get_place(&self, whg_id: &str) -> Result<Option<WhgPlaceMatch>, ConnectorError> {
        let token = self.config.api_token.as_ref().ok_or_else(|| {
            ConnectorError::NotConfigured("WHG_API_TOKEN environment variable not set".into())
        })?;

        let url = format!("{API_BASE}/places/{whg_id}/");
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Token {token}"))
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

        Ok(parse_whg_place(&payload))
    }
}

#[async_trait]
impl SourceConnector for WhgConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::Whg
    }

    fn connector_version(&self) -> &str {
        CONNECTOR_VERSION
    }

    async fn discover(
        &self,
        _subject: &ResolvedSubject,
        _cursor: Option<DiscoveryCursor>,
    ) -> Result<DiscoveryPage, ConnectorError> {
        // WHG is a place grounding service, not a document corpus.
        if !self.is_configured() {
            return Err(ConnectorError::NotConfigured(
                "WHG_API_TOKEN environment variable not set".into(),
            ));
        }
        Ok(DiscoveryPage {
            documents: vec![],
            next_cursor: None,
        })
    }

    async fn fetch(
        &self,
        _document: &DiscoveredDocument,
    ) -> Result<FetchedDocument, ConnectorError> {
        Err(ConnectorError::Unsupported(
            "WHG is a grounding service, not a document corpus".into(),
        ))
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        if !self.is_configured() {
            return Ok(ConnectorHealth {
                ok: false,
                detail: "WHG_API_TOKEN not configured".into(),
            });
        }

        // Try a simple search to verify connectivity
        match self.search_places("Paris").await {
            Ok(_) => Ok(ConnectorHealth {
                ok: true,
                detail: "WHG API reachable and authenticated".into(),
            }),
            Err(ConnectorError::NotConfigured(msg)) => Ok(ConnectorHealth {
                ok: false,
                detail: msg,
            }),
            Err(e) => Ok(ConnectorHealth {
                ok: false,
                detail: format!("WHG healthcheck failed: {e}"),
            }),
        }
    }
}

fn parse_whg_results(payload: &Value) -> Vec<WhgPlaceMatch> {
    let features = payload
        .get("features")
        .or_else(|| payload.get("results"))
        .and_then(|v| v.as_array());

    let Some(features) = features else {
        return vec![];
    };

    features.iter().filter_map(parse_whg_place).collect()
}

fn parse_whg_place(feature: &Value) -> Option<WhgPlaceMatch> {
    let props = feature.get("properties")?;
    let whg_id = props.get("place_id")?.to_string().trim_matches('"').to_string();
    let title = props.get("title")?.as_str()?.to_string();

    let ccodes: Vec<String> = props
        .get("ccodes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let types: Vec<String> = props
        .get("types")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    v.get("label")
                        .or_else(|| v.get("sourceLabel"))
                        .and_then(|l| l.as_str())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();

    let geom = feature.get("geometry").and_then(|g| {
        let r#type = g.get("type")?.as_str()?.to_string();
        let coordinates: Vec<f64> = g
            .get("coordinates")
            .and_then(|c| c.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
            .unwrap_or_default();
        if coordinates.is_empty() {
            None
        } else {
            Some(WhgGeometry { r#type, coordinates })
        }
    });

    let when = props.get("when").and_then(|w| {
        let timespans: Vec<WhgTimespan> = w
            .get("timespans")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|ts| {
                        Some(WhgTimespan {
                            start: ts.get("start")?.get("in")?.as_str().map(str::to_string),
                            end: ts.get("end").and_then(|e| e.get("in")).and_then(|i| i.as_str()).map(str::to_string),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        if timespans.is_empty() {
            None
        } else {
            Some(WhgTemporal { timespans })
        }
    });

    let links: Vec<WhgLink> = props
        .get("links")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|l| {
                    Some(WhgLink {
                        r#type: l.get("type")?.as_str()?.to_string(),
                        identifier: l.get("identifier")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Some(WhgPlaceMatch {
        whg_id,
        title,
        ccodes,
        types,
        geom,
        when,
        links,
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
    fn test_whg_config_from_env() {
        let config = WhgConfig::from_env();
        // Token will be None in test environment unless explicitly set
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn test_parse_empty_results() {
        let payload = serde_json::json!({ "features": [] });
        assert!(parse_whg_results(&payload).is_empty());
    }

    #[test]
    fn test_parse_whg_place() {
        let feature = serde_json::json!({
            "type": "Feature",
            "properties": {
                "place_id": 12345,
                "title": "Paris",
                "ccodes": ["FR"],
                "types": [{ "label": "city" }]
            },
            "geometry": {
                "type": "Point",
                "coordinates": [2.3522, 48.8566]
            }
        });
        let result = parse_whg_place(&feature).unwrap();
        assert_eq!(result.title, "Paris");
        assert_eq!(result.ccodes, vec!["FR"]);
        assert!(result.geom.is_some());
    }
}
