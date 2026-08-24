// crates/talaria-sources/src/connectors/wikisource.rs
use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use serde_json::Value;

use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::connectors::catalog::http_client;
use crate::kinds::{DiscoveryMethod, DocumentType, SourceKind};
use crate::plan::ResolvedSubject;
use crate::types::{DiscoveredDocument, ExternalEntityRef, SourceMetadata};

const API: &str = "https://fr.wikisource.org/w/api.php";
const CONNECTOR_VERSION: &str = "wikisource:fr_v1";
const SKIP_PREFIXES: &[&str] = &["page:", "livre:", "index:"];

/// Wikisource FR live connector (fr.wikisource.org Action API).
pub struct WikisourceConnector {
    pub http: reqwest::Client,
    pub max_docs: u32,
}

impl WikisourceConnector {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            http: http_client()?,
            max_docs: 15,
        })
    }

    pub fn document_from_title(title: &str) -> DiscoveredDocument {
        Self::parse_discover_from_sitelink(title)
    }

    pub fn parse_discover_from_sitelink(sitelink_title: &str) -> DiscoveredDocument {
        let genre = classify_genre(sitelink_title, "", &[]);
        let document_type = match genre {
            "letter" => DocumentType::Correspondence,
            other => DocumentType::Other(other.to_string()),
        };
        DiscoveredDocument {
            source_kind: SourceKind::Wikisource,
            external_id: sitelink_title.to_string(),
            canonical_url: Some(canonical_url(sitelink_title)),
            title: sitelink_title.to_string(),
            language: Some("fr".into()),
            document_type,
            subject_links: vec![ExternalEntityRef {
                system: "frwikisource".into(),
                id: sitelink_title.to_string(),
                label: Some(sitelink_title.to_string()),
            }],
            publication_time: None,
            discovery_method: DiscoveryMethod::IdentifierLookup,
            relevance_score: 0.9,
            source_metadata: SourceMetadata {
                raw: serde_json::json!({
                    "genre": genre,
                    "wiki": "frwikisource",
                }),
            },
        }
    }

    async fn search_titles(&self, label: &str) -> Result<Vec<String>, ConnectorError> {
        let limit = self.max_docs.to_string();
        let response = self
            .http
            .get(API)
            .query(&[
                ("action", "query"),
                ("list", "search"),
                ("srsearch", label),
                ("srnamespace", "0"),
                ("srlimit", limit.as_str()),
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
        Ok(parse_search_titles(&response))
    }

    async fn auteur_links(&self, label: &str) -> Result<Vec<String>, ConnectorError> {
        let titles = format!("Auteur:{label}");
        let response = self
            .http
            .get(API)
            .query(&[
                ("action", "query"),
                ("prop", "links"),
                ("titles", titles.as_str()),
                ("plnamespace", "0"),
                ("pllimit", "500"),
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
        Ok(parse_link_titles(&response))
    }

    async fn query_page(&self, title: &str) -> Result<Value, ConnectorError> {
        self.http
            .get(API)
            .query(&[
                ("action", "query"),
                ("prop", "revisions|info|pageprops"),
                ("rvprop", "content|ids"),
                ("rvslots", "main"),
                ("titles", title),
                ("format", "json"),
            ])
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .error_for_status()
            .map_err(|e| ConnectorError::Http(e.to_string()))?
            .json::<Value>()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))
    }
}

/// Map MediaWiki siteinfo namespace canonical `"*"` names to ids; skip empty main ns.
pub fn parse_siteinfo_namespaces(json: &Value) -> HashMap<String, i64> {
    let mut map = HashMap::new();
    let Some(namespaces) = json.pointer("/query/namespaces").and_then(Value::as_object) else {
        return map;
    };
    for ns in namespaces.values() {
        let name = ns.get("*").and_then(Value::as_str).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        if let Some(id) = ns.get("id").and_then(Value::as_i64) {
            map.insert(name.to_string(), id);
        }
    }
    map
}

/// Extract page titles from a MediaWiki search API response.
pub fn parse_search_titles(json: &Value) -> Vec<String> {
    json.pointer("/query/search")
        .and_then(Value::as_array)
        .map(|hits| {
            hits.iter()
                .filter_map(|hit| hit.get("title").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn fold_accents(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'À' | 'Â' | 'Ä' | 'Á' | 'Ã' => 'a',
            'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' | 'Ò' | 'Ó' | 'Ô' | 'Ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    let folded = fold_accents(haystack);
    needles.iter().any(|needle| folded.contains(needle))
}

/// Classify Wikisource document genre from title/categories (not event extraction).
pub fn classify_genre(title: &str, _wikitext: &str, categories: &[String]) -> &'static str {
    let combined = {
        let cats = categories.join(" ");
        if cats.is_empty() {
            title.to_string()
        } else {
            format!("{title} {cats}")
        }
    };

    if contains_any(&combined, &["lettre", "correspondance"]) {
        "letter"
    } else if contains_any(&combined, &["discours"]) {
        "speech"
    } else if contains_any(&combined, &["traite"]) {
        "treaty"
    } else if contains_any(&combined, &["loi", "code"]) {
        "law"
    } else if contains_any(&combined, &["memoire", "memoires"]) {
        "memoir"
    } else if contains_any(&combined, &["journal"]) {
        "periodical"
    } else {
        "narrative"
    }
}

fn canonical_url(title: &str) -> String {
    format!("https://fr.wikisource.org/wiki/{}", title.replace(' ', "_"))
}

fn is_skipped_discover_title(title: &str) -> bool {
    let folded = title.trim().to_ascii_lowercase();
    SKIP_PREFIXES
        .iter()
        .any(|prefix| folded.starts_with(prefix))
}

fn sitelink_titles(subject: &ResolvedSubject) -> (bool, Vec<String>) {
    let mut titles = Vec::new();
    let mut seen = HashSet::new();
    let mut had_sitelink = false;
    for (system, id) in &subject.known_identifiers {
        if !system.eq_ignore_ascii_case("frwikisource")
            && !system.eq_ignore_ascii_case("wikisource")
        {
            continue;
        }
        had_sitelink = true;
        if is_skipped_discover_title(id) || !seen.insert(id.clone()) {
            continue;
        }
        titles.push(id.clone());
    }
    (had_sitelink, titles)
}

fn parse_link_titles(json: &Value) -> Vec<String> {
    let Some(pages) = json.pointer("/query/pages").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut titles = Vec::new();
    for page in pages.values() {
        let Some(links) = page.get("links").and_then(Value::as_array) else {
            continue;
        };
        for link in links {
            if let Some(title) = link.get("title").and_then(Value::as_str) {
                titles.push(title.to_string());
            }
        }
    }
    titles
}

fn revision_wikitext(page: &Value) -> Option<String> {
    let rev = page.get("revisions")?.as_array()?.first()?;
    let from_slot = rev
        .pointer("/slots/main")
        .and_then(|slot| slot.get("content").or_else(|| slot.get("*")))
        .and_then(Value::as_str);
    let from_rev = rev
        .get("content")
        .or_else(|| rev.get("*"))
        .and_then(Value::as_str);
    from_slot.or(from_rev).map(str::to_string)
}

/// Parse a Wikisource Action API page JSON into wikitext + fetch metadata.
pub fn parse_fetch_page(json: &Value) -> Option<(String, Value)> {
    let pages = json.pointer("/query/pages")?.as_object()?;
    let page = pages.values().next()?;
    if page.get("missing").is_some() {
        return None;
    }
    let text = revision_wikitext(page).unwrap_or_default();
    let page_id = page.get("pageid").cloned().unwrap_or(Value::Null);
    let revision_id = page
        .get("lastrevid")
        .cloned()
        .or_else(|| {
            page.get("revisions")
                .and_then(Value::as_array)
                .and_then(|revs| revs.first())
                .and_then(|rev| rev.get("revid"))
                .cloned()
        })
        .unwrap_or(Value::Null);
    let qid = page
        .pointer("/pageprops/wikibase_item")
        .cloned()
        .unwrap_or(Value::Null);
    let namespace = page.get("ns").cloned().unwrap_or(Value::Null);
    let metadata = serde_json::json!({
        "page_id": page_id,
        "revision_id": revision_id,
        "qid": qid,
        "wiki": "frwikisource",
        "namespace": namespace,
    });
    Some((text, metadata))
}

fn fallback_index_livre_titles(title: &str) -> Vec<String> {
    if is_skipped_discover_title(title) {
        return Vec::new();
    }
    vec![format!("Index:{title}"), format!("Livre:{title}")]
}

/// Merge live search + Auteur: link results when no sitelink identifiers exist.
fn merge_live_discover(
    search: Result<Vec<String>, ConnectorError>,
    links: Result<Vec<String>, ConnectorError>,
) -> Result<Vec<String>, ConnectorError> {
    let search_ok = search.is_ok();
    let links_ok = links.is_ok();
    if !search_ok && !links_ok {
        return Err(search.err().unwrap_or_else(|| links.unwrap_err()));
    }

    let mut titles = Vec::new();
    let mut seen = HashSet::new();
    for hits in [search, links].into_iter().filter_map(Result::ok) {
        for title in hits {
            if seen.insert(title.clone()) {
                titles.push(title);
            }
        }
    }
    Ok(titles)
}

#[async_trait]
impl SourceConnector for WikisourceConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::Wikisource
    }

    fn connector_version(&self) -> &str {
        CONNECTOR_VERSION
    }

    async fn discover(
        &self,
        subject: &ResolvedSubject,
        cursor: Option<DiscoveryCursor>,
    ) -> Result<DiscoveryPage, ConnectorError> {
        if cursor.map(|c| c.offset).unwrap_or(0) > 0 {
            return Ok(DiscoveryPage {
                documents: vec![],
                next_cursor: None,
            });
        }

        let (had_sitelink, mut titles) = sitelink_titles(subject);
        if titles.is_empty() && !had_sitelink {
            let search = self.search_titles(&subject.label).await;
            let links = self.auteur_links(&subject.label).await;
            titles = merge_live_discover(search, links)?;
            titles.retain(|title| !is_skipped_discover_title(title));
        }

        titles.truncate(self.max_docs as usize);
        let documents = titles
            .into_iter()
            .map(|title| Self::document_from_title(&title))
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
        let json = self.query_page(&document.title).await?;
        let (mut text, mut metadata) = parse_fetch_page(&json)
            .ok_or_else(|| ConnectorError::Parse(format!("missing page {}", document.title)))?;
        if text.trim().is_empty() {
            for fallback in fallback_index_livre_titles(&document.title) {
                let Ok(fb_json) = self.query_page(&fallback).await else {
                    continue;
                };
                let Some((fb_text, fb_meta)) = parse_fetch_page(&fb_json) else {
                    continue;
                };
                if !fb_text.trim().is_empty() {
                    text = fb_text;
                    metadata = fb_meta;
                    break;
                }
            }
        }
        let revision_id = metadata.get("revision_id").and_then(|v| match v {
            Value::Number(n) => Some(n.to_string()),
            Value::String(s) => Some(s.clone()),
            _ => None,
        });
        let bytes = text.len() as u64;
        Ok(FetchedDocument {
            discovered: document.clone(),
            revision_id,
            content_type: "text/x-wiki".into(),
            text,
            raw_metadata: metadata,
            license: Some("CC BY-SA".into()),
            content_bytes: bytes,
        })
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        Ok(ConnectorHealth {
            ok: true,
            detail: "fr.wikisource.org Action API".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connector::SourceConnector;
    use crate::kinds::SourceKind;
    use crate::plan::ResolvedSubject;

    #[test]
    fn siteinfo_finds_page_ns_without_hardcoded_number() {
        let json = serde_json::json!({"query":{"namespaces":{
            "0": {"id": 0, "*": ""},
            "104": {"id": 104, "*": "Page"},
            "114": {"id": 114, "*": "Livre"}
        }}});
        let map = parse_siteinfo_namespaces(&json);
        assert_eq!(map.get("Page").copied(), Some(104));
        assert_eq!(map.get("Livre").copied(), Some(114));
        assert!(!map.contains_key(""));
    }

    #[test]
    fn parse_search_titles_extracts_from_query_search() {
        let json = serde_json::json!({
            "query": {
                "search": [
                    {"title": "Foo"},
                    {"title": "Bar"}
                ]
            }
        });
        assert_eq!(
            parse_search_titles(&json),
            vec!["Foo".to_string(), "Bar".to_string()]
        );
    }

    #[test]
    fn genre_letter() {
        assert_eq!(classify_genre("Lettre à Joséphine", "", &[]), "letter");
    }

    #[test]
    fn genre_speech() {
        assert_eq!(
            classify_genre("Discours aux états généraux", "", &[]),
            "speech"
        );
    }

    #[test]
    fn genre_treaty_accented() {
        assert_eq!(classify_genre("Traité de Campoformio", "", &[]), "treaty");
    }

    #[test]
    fn genre_treaty_unaccented() {
        assert_eq!(classify_genre("Traite de Campoformio", "", &[]), "treaty");
    }

    #[test]
    fn genre_law() {
        assert_eq!(classify_genre("Code civil", "", &[]), "law");
        assert_eq!(classify_genre("Loi sur les successions", "", &[]), "law");
    }

    #[test]
    fn genre_memoir_accented() {
        assert_eq!(
            classify_genre("Mémoires sur la Révolution", "", &[]),
            "memoir"
        );
    }

    #[test]
    fn genre_memoir_unaccented() {
        assert_eq!(
            classify_genre("Memoires sur la Revolution", "", &[]),
            "memoir"
        );
    }

    #[test]
    fn genre_periodical() {
        assert_eq!(classify_genre("Journal des débats", "", &[]), "periodical");
    }

    #[test]
    fn genre_narrative_default() {
        assert_eq!(classify_genre("Histoire de France", "", &[]), "narrative");
    }

    #[test]
    fn genre_from_category() {
        assert_eq!(
            classify_genre(
                "Correspondance secrète",
                "",
                &["Lettres de Napoléon".into()]
            ),
            "letter"
        );
    }

    #[test]
    fn sitelink_becomes_document() {
        let d = WikisourceConnector::document_from_title("Correspondance de Napoléon");
        assert_eq!(d.source_kind, SourceKind::Wikisource);
        assert!(d.canonical_url.unwrap().contains("wikisource.org"));
    }

    fn napoleon_with_ids(ids: Vec<(String, String)>) -> ResolvedSubject {
        ResolvedSubject {
            entity_id: None,
            qid: Some("Q517".into()),
            label: "Napoléon".into(),
            languages: vec!["fr".into()],
            birth_year: Some(1769),
            death_year: Some(1821),
            countries: vec!["France".into()],
            occupations: vec![],
            known_identifiers: ids,
        }
    }

    #[tokio::test]
    async fn discover_skips_page_livre_index_titles() {
        let conn = WikisourceConnector::new().unwrap();
        let subject = napoleon_with_ids(vec![
            ("frwikisource".into(), "Page:Foo.djvu".into()),
            ("wikisource".into(), "Livre:Bar".into()),
            ("frwikisource".into(), "Index:Baz".into()),
            ("frwikisource".into(), "page:lower.djvu".into()),
            ("frwikisource".into(), "Correspondance de Napoléon".into()),
        ]);
        let page = conn.discover(&subject, None).await.unwrap();
        assert_eq!(page.documents.len(), 1);
        assert_eq!(page.documents[0].title, "Correspondance de Napoléon");
    }

    #[tokio::test]
    async fn injected_identifier_discover() {
        let conn = WikisourceConnector::new().unwrap();
        let subject = napoleon_with_ids(vec![(
            "frwikisource".into(),
            "Correspondance de Napoléon".into(),
        )]);
        let page = conn.discover(&subject, None).await.unwrap();
        assert_eq!(page.documents.len(), 1);
        assert_eq!(page.documents[0].source_kind, SourceKind::Wikisource);
        assert!(
            page.documents[0]
                .canonical_url
                .as_ref()
                .unwrap()
                .contains("fr.wikisource.org")
        );
        assert!(
            page.documents[0]
                .canonical_url
                .as_ref()
                .unwrap()
                .contains("Correspondance_de_Napoléon")
        );
    }

    #[test]
    fn parse_fetch_page_reads_slot_content() {
        let json = serde_json::json!({
            "query": {
                "pages": {
                    "42": {
                        "pageid": 42,
                        "ns": 0,
                        "lastrevid": 99,
                        "pageprops": {"wikibase_item": "Q123"},
                        "revisions": [{
                            "revid": 99,
                            "slots": {"main": {"content": "Bonjour Wikisource"}}
                        }]
                    }
                }
            }
        });
        let (text, meta) = parse_fetch_page(&json).unwrap();
        assert_eq!(text, "Bonjour Wikisource");
        assert_eq!(meta["page_id"], 42);
        assert_eq!(meta["revision_id"], 99);
        assert_eq!(meta["qid"], "Q123");
        assert_eq!(meta["wiki"], "frwikisource");
        assert_eq!(meta["namespace"], 0);
    }

    #[test]
    fn parse_fetch_page_empty_transclusion() {
        let json = serde_json::json!({
            "query": {
                "pages": {
                    "1": {
                        "pageid": 1,
                        "ns": 0,
                        "revisions": [{"slots": {"main": {"content": ""}}}]
                    }
                }
            }
        });
        let (text, meta) = parse_fetch_page(&json).unwrap();
        assert!(text.is_empty());
        assert_eq!(meta["wiki"], "frwikisource");
    }

    fn http_err(msg: &str) -> ConnectorError {
        ConnectorError::Http(msg.into())
    }

    #[test]
    fn merge_live_discover_both_err_returns_first_err() {
        let err = merge_live_discover(
            Err(http_err("search down")),
            Err(http_err("links down")),
        );
        assert!(matches!(err, Err(ConnectorError::Http(ref m)) if m == "search down"));
    }

    #[test]
    fn merge_live_discover_one_ok_with_titles_keeps_titles() {
        let titles = merge_live_discover(
            Ok(vec!["Correspondance".into()]),
            Err(http_err("links down")),
        )
        .unwrap();
        assert_eq!(titles, vec!["Correspondance".to_string()]);
    }

    #[test]
    fn merge_live_discover_both_ok_empty_returns_empty() {
        let titles = merge_live_discover(Ok(vec![]), Ok(vec![])).unwrap();
        assert!(titles.is_empty());
    }

    #[test]
    fn live_registry_marks_wikisource_implemented() {
        let reg = crate::connectors::default_registry(None, true).unwrap();
        let entry = reg
            .get(&SourceKind::Wikisource)
            .expect("wikisource registered");
        assert!(entry.implemented);
    }
}
