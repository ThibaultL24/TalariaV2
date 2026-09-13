// crates/talaria-sources/src/connectors/france_archives.rs
//! FranceArchives connector for structured life facts.
//!
//! FranceArchives aggregates French archival metadata (persons, institutions, events).
//! Useful for birth/death/office facts from EAD/EAC-CPF records.
//!
//! Docs: https://francearchives.gouv.fr/
//! API: SPARQL endpoint at https://francearchives.gouv.fr/sparql

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

pub const CONNECTOR_VERSION: &str = "france_archives:v1";
const SPARQL_ENDPOINT: &str = "https://francearchives.gouv.fr/sparql";
const USER_AGENT: &str = "TalariaEngine/0.1 (+archives; fa connector)";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaPersonRecord {
    pub uri: String,
    pub label: String,
    pub birth_date: Option<String>,
    pub birth_place: Option<String>,
    pub death_date: Option<String>,
    pub death_place: Option<String>,
    pub occupations: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FranceArchivesConfig {
    pub timeout_secs: u64,
}

pub struct FranceArchivesConnector {
    http: reqwest::Client,
    config: FranceArchivesConfig,
}

impl FranceArchivesConnector {
    pub fn new(config: FranceArchivesConfig) -> anyhow::Result<Self> {
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

    pub async fn search_person(&self, name: &str) -> Result<Vec<FaPersonRecord>, ConnectorError> {
        let query = person_search_query(name);
        let payload = self.sparql_json(&query).await?;
        Ok(parse_person_results(&payload))
    }

    async fn sparql_json(&self, query: &str) -> Result<Value, ConnectorError> {
        let resp = self
            .http
            .post(SPARQL_ENDPOINT)
            .header("Accept", "application/sparql-results+json")
            .header("Content-Type", "application/sparql-query")
            .body(query.to_string())
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
impl SourceConnector for FranceArchivesConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::FranceArchives
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
            .filter_map(|r| fa_record_to_discovered(&r, subject))
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
        // FranceArchives records are fetched via SPARQL in discover.
        // Full fetch could query individual record URI for more detail.
        let text = format!(
            "FranceArchives record: {}\n{}",
            document.title,
            document.canonical_url.as_deref().unwrap_or("")
        );
        Ok(FetchedDocument {
            discovered: document.clone(),
            revision_id: None,
            content_type: "text/plain".into(),
            text,
            raw_metadata: document.source_metadata.raw.clone(),
            license: Some("Licence Ouverte 2.0".into()),
            content_bytes: 0,
        })
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        let query = "SELECT (1 AS ?ok) WHERE {} LIMIT 1";
        match self.sparql_json(query).await {
            Ok(_) => Ok(ConnectorHealth {
                ok: true,
                detail: "FranceArchives SPARQL endpoint reachable".into(),
            }),
            Err(e) => Ok(ConnectorHealth {
                ok: false,
                detail: format!("FranceArchives healthcheck failed: {e}"),
            }),
        }
    }
}

fn person_search_query(name: &str) -> String {
    let escaped = sparql_escape(name);
    format!(
        r#"PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX bio: <http://purl.org/vocab/bio/0.1/>
PREFIX schema: <http://schema.org/>

SELECT DISTINCT ?person ?label ?birthDate ?birthPlace ?deathDate ?deathPlace ?occupation ?description WHERE {{
  ?person a foaf:Person ;
          rdfs:label ?label .
  FILTER(CONTAINS(LCASE(?label), LCASE("{escaped}")))
  OPTIONAL {{ ?person bio:birth/bio:date ?birthDate . }}
  OPTIONAL {{ ?person bio:birth/bio:place/rdfs:label ?birthPlace . }}
  OPTIONAL {{ ?person bio:death/bio:date ?deathDate . }}
  OPTIONAL {{ ?person bio:death/bio:place/rdfs:label ?deathPlace . }}
  OPTIONAL {{ ?person schema:hasOccupation/rdfs:label ?occupation . }}
  OPTIONAL {{ ?person schema:description ?description . }}
}}
LIMIT 50"#
    )
}

fn parse_person_results(payload: &Value) -> Vec<FaPersonRecord> {
    let Some(bindings) = payload.pointer("/results/bindings").and_then(|v| v.as_array()) else {
        return vec![];
    };

    let mut records: std::collections::HashMap<String, FaPersonRecord> = std::collections::HashMap::new();

    for row in bindings {
        let Some(uri) = binding_str(row, "person") else { continue };
        let label = binding_str(row, "label").unwrap_or_default();

        let entry = records.entry(uri.clone()).or_insert_with(|| FaPersonRecord {
            uri: uri.clone(),
            label,
            birth_date: None,
            birth_place: None,
            death_date: None,
            death_place: None,
            occupations: vec![],
            description: None,
        });

        if entry.birth_date.is_none() {
            entry.birth_date = binding_str(row, "birthDate");
        }
        if entry.birth_place.is_none() {
            entry.birth_place = binding_str(row, "birthPlace");
        }
        if entry.death_date.is_none() {
            entry.death_date = binding_str(row, "deathDate");
        }
        if entry.death_place.is_none() {
            entry.death_place = binding_str(row, "deathPlace");
        }
        if entry.description.is_none() {
            entry.description = binding_str(row, "description");
        }
        if let Some(occ) = binding_str(row, "occupation") {
            if !entry.occupations.contains(&occ) {
                entry.occupations.push(occ);
            }
        }
    }

    records.into_values().collect()
}

fn fa_record_to_discovered(
    record: &FaPersonRecord,
    subject: &ResolvedSubject,
) -> Option<DiscoveredDocument> {
    // Basic name matching
    let record_lower = record.label.to_lowercase();
    let subject_lower = subject.label.to_lowercase();
    if !record_lower.contains(&subject_lower) && !subject_lower.contains(&record_lower) {
        return None;
    }

    let year = record
        .birth_date
        .as_ref()
        .and_then(|d| d.get(..4))
        .and_then(|y| y.parse::<i32>().ok());

    Some(DiscoveredDocument {
        source_kind: SourceKind::FranceArchives,
        external_id: record.uri.rsplit('/').next().unwrap_or(&record.uri).to_string(),
        canonical_url: Some(record.uri.clone()),
        title: record.label.clone(),
        language: Some("fr".into()),
        document_type: DocumentType::AuthorityRecord,
        subject_links: vec![],
        publication_time: year.map(|y| TypedTimeLite::Exact {
            year: y,
            surface: record.birth_date.clone(),
        }),
        discovery_method: DiscoveryMethod::Sparql,
        relevance_score: 0.7,
        source_metadata: SourceMetadata {
            raw: serde_json::json!({
                "birth_date": record.birth_date,
                "birth_place": record.birth_place,
                "death_date": record.death_date,
                "death_place": record.death_place,
                "occupations": record.occupations,
            }),
        },
    })
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
    fn test_person_search_query() {
        let query = person_search_query("Napoleon");
        assert!(query.contains("CONTAINS(LCASE(?label), LCASE(\"Napoleon\"))"));
    }

    #[test]
    fn test_parse_empty_results() {
        let payload = serde_json::json!({ "results": { "bindings": [] } });
        assert!(parse_person_results(&payload).is_empty());
    }
}
