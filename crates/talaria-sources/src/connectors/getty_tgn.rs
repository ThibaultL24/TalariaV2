// crates/talaria-sources/src/connectors/getty_tgn.rs
//! Getty Thesaurus of Geographic Names (TGN) connector.
//!
//! TGN provides place-identity grounding via SPARQL at vocab.getty.edu.
//! This connector resolves place names to TGN URIs and retrieves place hierarchies
//! (e.g., Paris → Île-de-France → France). Coordinates come from P625/gazetteer,
//! not from TGN directly.
//!
//! Docs: https://www.getty.edu/research/tools/vocabularies/tgn/
//! SPARQL endpoint: http://vocab.getty.edu/sparql

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

pub const CONNECTOR_VERSION: &str = "getty_tgn:v1";
const SPARQL_ENDPOINT: &str = "http://vocab.getty.edu/sparql";
const USER_AGENT: &str = "TalariaEngine/0.1 (+grounding; tgn connector)";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TgnPlaceMatch {
    pub tgn_uri: String,
    pub tgn_id: String,
    pub label: String,
    pub place_type: Option<String>,
    pub parent_label: Option<String>,
    pub parent_tgn_id: Option<String>,
    pub broader_hierarchy: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct GettyTgnConfig {
    pub timeout_secs: u64,
}

pub struct GettyTgnConnector {
    http: reqwest::Client,
    config: GettyTgnConfig,
}

impl GettyTgnConnector {
    pub fn new(config: GettyTgnConfig) -> anyhow::Result<Self> {
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

    pub async fn resolve_place(&self, place_name: &str) -> Result<Vec<TgnPlaceMatch>, ConnectorError> {
        let query = place_search_query(place_name);
        let payload = self.sparql_json(&query).await?;
        Ok(parse_place_results(&payload))
    }

    pub async fn get_place_hierarchy(&self, tgn_id: &str) -> Result<Vec<String>, ConnectorError> {
        let query = hierarchy_query(tgn_id);
        let payload = self.sparql_json(&query).await?;
        Ok(parse_hierarchy(&payload))
    }

    async fn sparql_json(&self, query: &str) -> Result<Value, ConnectorError> {
        let resp = self
            .http
            .get(SPARQL_ENDPOINT)
            .query(&[("query", query), ("format", "application/sparql-results+json")])
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

        resp.json()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))
    }
}

#[async_trait]
impl SourceConnector for GettyTgnConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::GettyTgn
    }

    fn connector_version(&self) -> &str {
        CONNECTOR_VERSION
    }

    async fn discover(
        &self,
        _subject: &ResolvedSubject,
        _cursor: Option<DiscoveryCursor>,
    ) -> Result<DiscoveryPage, ConnectorError> {
        // TGN is a place grounding service, not a document corpus.
        // Discovery returns empty; use resolve_place() for grounding queries.
        Ok(DiscoveryPage {
            documents: vec![],
            next_cursor: None,
        })
    }

    async fn fetch(
        &self,
        _document: &DiscoveredDocument,
    ) -> Result<FetchedDocument, ConnectorError> {
        // TGN entries are fetched via SPARQL, not as documents.
        Err(ConnectorError::Unsupported(
            "TGN is a grounding service, not a document corpus".into(),
        ))
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        let query = "SELECT (1 AS ?ok) WHERE {} LIMIT 1";
        match self.sparql_json(query).await {
            Ok(_) => Ok(ConnectorHealth {
                ok: true,
                detail: "Getty TGN SPARQL endpoint reachable".into(),
            }),
            Err(e) => Ok(ConnectorHealth {
                ok: false,
                detail: format!("TGN healthcheck failed: {e}"),
            }),
        }
    }
}

fn place_search_query(place_name: &str) -> String {
    let escaped = sparql_escape(place_name);
    format!(
        r#"PREFIX gvp: <http://vocab.getty.edu/ontology#>
PREFIX skos: <http://www.w3.org/2004/02/skos/core#>
PREFIX skosxl: <http://www.w3.org/2008/05/skos-xl#>
PREFIX dct: <http://purl.org/dc/terms/>

SELECT ?subject ?label ?placeType ?parentLabel ?parentSubject WHERE {{
  ?subject a gvp:Subject ;
           skos:inScheme <http://vocab.getty.edu/tgn/> ;
           gvp:prefLabelGVP/skosxl:literalForm ?label ;
           gvp:placeTypePreferred/gvp:prefLabelGVP/skosxl:literalForm ?placeType .
  OPTIONAL {{
    ?subject gvp:broaderPreferred ?parentSubject .
    ?parentSubject gvp:prefLabelGVP/skosxl:literalForm ?parentLabel .
  }}
  FILTER(CONTAINS(LCASE(?label), LCASE("{escaped}")))
}}
LIMIT 20"#
    )
}

fn hierarchy_query(tgn_id: &str) -> String {
    let tgn_uri = if tgn_id.starts_with("http") {
        tgn_id.to_string()
    } else {
        format!("http://vocab.getty.edu/tgn/{}", tgn_id.trim_start_matches("tgn:"))
    };
    format!(
        r#"PREFIX gvp: <http://vocab.getty.edu/ontology#>
PREFIX skosxl: <http://www.w3.org/2008/05/skos-xl#>

SELECT ?ancestor ?label WHERE {{
  <{tgn_uri}> gvp:broaderPreferred* ?ancestor .
  ?ancestor gvp:prefLabelGVP/skosxl:literalForm ?label .
}}
ORDER BY DESC(?ancestor)"#
    )
}

fn parse_place_results(payload: &Value) -> Vec<TgnPlaceMatch> {
    let Some(bindings) = payload.pointer("/results/bindings").and_then(|v| v.as_array()) else {
        return vec![];
    };
    
    bindings
        .iter()
        .filter_map(|row| {
            let tgn_uri = binding_str(row, "subject")?;
            let tgn_id = tgn_uri.rsplit('/').next()?.to_string();
            let label = binding_str(row, "label")?;
            let place_type = binding_str(row, "placeType");
            let parent_label = binding_str(row, "parentLabel");
            let parent_tgn_id = binding_str(row, "parentSubject")
                .and_then(|uri| uri.rsplit('/').next().map(str::to_string));
            
            Some(TgnPlaceMatch {
                tgn_uri,
                tgn_id,
                label,
                place_type,
                parent_label,
                parent_tgn_id,
                broader_hierarchy: vec![],
            })
        })
        .collect()
}

fn parse_hierarchy(payload: &Value) -> Vec<String> {
    let Some(bindings) = payload.pointer("/results/bindings").and_then(|v| v.as_array()) else {
        return vec![];
    };
    
    bindings
        .iter()
        .filter_map(|row| binding_str(row, "label"))
        .collect()
}

fn binding_str(row: &Value, key: &str) -> Option<String> {
    row.get(key)
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn sparql_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
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
    fn test_place_search_query() {
        let query = place_search_query("Paris");
        assert!(query.contains("CONTAINS(LCASE(?label), LCASE(\"Paris\"))"));
    }

    #[test]
    fn test_hierarchy_query() {
        let query = hierarchy_query("7008038");
        assert!(query.contains("http://vocab.getty.edu/tgn/7008038"));
    }

    #[test]
    fn test_parse_place_results_empty() {
        let payload = serde_json::json!({ "results": { "bindings": [] } });
        assert!(parse_place_results(&payload).is_empty());
    }

    #[test]
    fn test_parse_place_results() {
        let payload = serde_json::json!({
            "results": {
                "bindings": [{
                    "subject": { "value": "http://vocab.getty.edu/tgn/7008038" },
                    "label": { "value": "Paris" },
                    "placeType": { "value": "inhabited place" },
                    "parentLabel": { "value": "Île-de-France" },
                    "parentSubject": { "value": "http://vocab.getty.edu/tgn/7002883" }
                }]
            }
        });
        let results = parse_place_results(&payload);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].label, "Paris");
        assert_eq!(results[0].tgn_id, "7008038");
        assert_eq!(results[0].parent_label.as_deref(), Some("Île-de-France"));
    }
}
