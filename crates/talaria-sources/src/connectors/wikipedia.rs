// crates/talaria-sources/src/connectors/wikipedia.rs
use async_trait::async_trait;
use serde_json::Value;

use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::kinds::{DiscoveryMethod, DocumentType, SourceKind};
use crate::plan::ResolvedSubject;
use crate::types::{DiscoveredDocument, ExternalEntityRef, SourceMetadata};

const UA: &str = "TalariaEngine/0.1 (https://github.com/talaria; multi-source ingest)";

#[derive(Debug, Clone)]
pub struct WikipediaConnectorConfig {
    pub languages: Vec<String>,
    pub max_linked_pages: u32,
}

impl Default for WikipediaConnectorConfig {
    fn default() -> Self {
        Self {
            languages: vec!["en".into(), "fr".into()],
            max_linked_pages: 12,
        }
    }
}

pub struct WikipediaConnector {
    http: reqwest::Client,
    config: WikipediaConnectorConfig,
}

impl WikipediaConnector {
    pub fn new(config: WikipediaConnectorConfig) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder().user_agent(UA).build()?;
        Ok(Self { http, config })
    }

    fn api(lang: &str) -> String {
        format!("https://{lang}.wikipedia.org/w/api.php")
    }

    async fn fetch_extract(
        &self,
        lang: &str,
        title: &str,
    ) -> Result<(String, Value), ConnectorError> {
        let response = self
            .http
            .get(Self::api(lang))
            .query(&[
                ("action", "query"),
                ("prop", "extracts|info|pageprops|revisions"),
                ("explaintext", "1"),
                ("exlimit", "1"),
                ("rvprop", "content|ids"),
                ("rvslots", "main"),
                ("titles", title),
                ("format", "json"),
                ("redirects", "1"),
            ])
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .error_for_status()
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .json::<Value>()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))?;

        let pages = response
            .pointer("/query/pages")
            .and_then(|v| v.as_object())
            .ok_or_else(|| ConnectorError::Parse("no pages".into()))?;
        let page = pages
            .values()
            .next()
            .ok_or_else(|| ConnectorError::Parse("empty pages".into()))?;
        if page.get("missing").is_some() {
            return Err(ConnectorError::Parse(format!("missing page {title}")));
        }
        let extract = page
            .get("extract")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok(select_fetch_text(page.clone(), extract))
    }

    async fn search_titles(
        &self,
        lang: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<String>, ConnectorError> {
        if limit == 0 || query.trim().is_empty() {
            return Ok(vec![]);
        }
        let response = self
            .http
            .get(Self::api(lang))
            .query(&[
                ("action", "query"),
                ("list", "search"),
                ("srsearch", query),
                ("srlimit", &limit.min(20).to_string()),
                ("srnamespace", "0"),
                ("format", "json"),
            ])
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .error_for_status()
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .json::<Value>()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))?;
        let hits = response
            .pointer("/query/search")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(hits
            .iter()
            .filter_map(|h| h.get("title").and_then(|t| t.as_str()).map(str::to_string))
            .collect())
    }
}

#[async_trait]
impl SourceConnector for WikipediaConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::Wikipedia
    }

    fn connector_version(&self) -> &str {
        "wikipedia:extracts_v1"
    }

    async fn discover(
        &self,
        subject: &ResolvedSubject,
        cursor: Option<DiscoveryCursor>,
    ) -> Result<DiscoveryPage, ConnectorError> {
        let offset = cursor.map(|c| c.offset).unwrap_or(0);
        if offset > 0 {
            return Ok(DiscoveryPage {
                documents: vec![],
                next_cursor: None,
            });
        }
        let search_q = subject.label.clone();
        let mut docs = Vec::new();
        for lang in &self.config.languages {
            let title = subject.label.clone();
            docs.push(DiscoveredDocument {
                source_kind: SourceKind::Wikipedia,
                external_id: format!("{lang}:{title}"),
                canonical_url: Some(format!(
                    "https://{lang}.wikipedia.org/wiki/{}",
                    title.replace(' ', "_")
                )),
                title: title.clone(),
                language: Some(lang.clone()),
                document_type: DocumentType::Article,
                subject_links: vec![ExternalEntityRef {
                    system: "wikipedia".into(),
                    id: format!("{lang}:{title}"),
                    label: Some(title.clone()),
                }],
                publication_time: None,
                discovery_method: DiscoveryMethod::SubjectSearch,
                relevance_score: 0.95,
                source_metadata: SourceMetadata {
                    raw: serde_json::json!({"lang": lang}),
                },
            });
            let extra = self
                .search_titles(lang, &search_q, self.config.max_linked_pages as usize)
                .await
                .unwrap_or_default();
            for hit in extra {
                if hit.eq_ignore_ascii_case(&subject.label) || crate::is_noise_wiki_title(&hit) {
                    continue;
                }
                docs.push(DiscoveredDocument {
                    source_kind: SourceKind::Wikipedia,
                    external_id: format!("{lang}:{hit}"),
                    canonical_url: Some(format!(
                        "https://{lang}.wikipedia.org/wiki/{}",
                        hit.replace(' ', "_")
                    )),
                    title: hit.clone(),
                    language: Some(lang.clone()),
                    document_type: DocumentType::Article,
                    subject_links: vec![ExternalEntityRef {
                        system: "wikipedia".into(),
                        id: format!("{lang}:{hit}"),
                        label: Some(hit),
                    }],
                    publication_time: None,
                    discovery_method: DiscoveryMethod::LinkedEntity,
                    relevance_score: 0.8,
                    source_metadata: SourceMetadata {
                        raw: serde_json::json!({"lang": lang, "via": "wiki_search"}),
                    },
                });
            }
        }
        Ok(DiscoveryPage {
            documents: docs,
            next_cursor: None,
        })
    }

    async fn fetch(
        &self,
        document: &DiscoveredDocument,
    ) -> Result<FetchedDocument, ConnectorError> {
        let lang = document.language.as_deref().unwrap_or("en");
        let title = document.title.as_str();
        let (text, page) = self.fetch_extract(lang, title).await?;
        let revid = page
            .get("lastrevid")
            .and_then(|v| v.as_u64())
            .map(|n| n.to_string());
        let bytes = text.len() as u64;
        let content_type = if page.get("source_form").and_then(|v| v.as_str()) == Some("plain") {
            "text/plain"
        } else {
            "text/x-wiki"
        };
        Ok(FetchedDocument {
            discovered: document.clone(),
            revision_id: revid,
            content_type: content_type.into(),
            text,
            raw_metadata: page,
            license: Some("CC BY-SA".into()),
            content_bytes: bytes,
        })
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        Ok(ConnectorHealth {
            ok: true,
            detail: format!("langs={:?}", self.config.languages),
        })
    }
}

fn revision_wikitext(page: &Value) -> Option<String> {
    let rev = page.get("revisions")?.as_array()?.first()?;
    let from_slot = rev
        .pointer("/slots/main")
        .and_then(|slot| slot.get("content").or_else(|| slot.get("*")))
        .and_then(|v| v.as_str());
    let from_rev = rev
        .get("content")
        .or_else(|| rev.get("*"))
        .and_then(|v| v.as_str());
    from_slot
        .or(from_rev)
        .map(str::to_string)
        .filter(|s| !s.is_empty())
}

fn select_fetch_text(mut page: Value, extract: String) -> (String, Value) {
    page["plain_extract"] = Value::String(extract.clone());
    if let Some(wikitext) = revision_wikitext(&page) {
        (wikitext, page)
    } else {
        page["source_form"] = Value::String("plain".into());
        (extract, page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prefers_revision_wikitext_over_extract() {
        let page = json!({
            "extract": "plain biography",
            "pageprops": {"wikibase_item": "Q517"},
            "revisions": [{
                "revid": 1,
                "slots": {"main": {"content": "== Life ==\nHe was born in Ajaccio."}}
            }]
        });
        let (text, meta) = select_fetch_text(page, "plain biography".into());
        assert_eq!(text, "== Life ==\nHe was born in Ajaccio.");
        assert_eq!(meta["plain_extract"], "plain biography");
        assert_eq!(meta["pageprops"]["wikibase_item"], "Q517");
        assert!(meta.get("source_form").is_none());
    }

    #[test]
    fn falls_back_to_plain_extract_when_wikitext_missing() {
        let page = json!({
            "extract": "plain biography",
            "pageprops": {"wikibase_item": "Q517"}
        });
        let (text, meta) = select_fetch_text(page, "plain biography".into());
        assert_eq!(text, "plain biography");
        assert_eq!(meta["source_form"], "plain");
        assert_eq!(meta["plain_extract"], "plain biography");
    }
}
