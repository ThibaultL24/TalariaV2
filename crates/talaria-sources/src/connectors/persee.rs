// crates/talaria-sources/src/connectors/persee.rs
//! Persée portal search + OAI-PMH fetch.
//!
//! Discovery uses the public search UI (`/search?q=…&searchDomain=documents`) with
//! profile-aware query buckets (label + class terms), inspired by the old repo's
//! subject+topic deep search. Fetch uses OAI-PMH GetRecord.

use async_trait::async_trait;
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::connectors::catalog::http_client;
use crate::corpus::{NormalizedCorpusDocument, NormalizedSubject};
use crate::kinds::{
    AcademicStatus, AccessLevel, ContributionRole, DiscoveryMethod, DocumentType, SourceKind,
};
use crate::plan::ResolvedSubject;
use crate::types::{DiscoveredDocument, SourceMetadata, TypedTimeLite};

pub const CONNECTOR_VERSION: &str = "persee:v2";
const OAI_BASE: &str = "http://oai.persee.fr/oai";
const SEARCH_BASE: &str = "https://www.persee.fr/search";

pub struct PerseeConnector {
    http: reqwest::Client,
    max_docs: u32,
}

#[derive(Debug, Clone)]
struct PortalHit {
    slug: String,
    title: String,
}

impl PerseeConnector {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            http: http_client()?,
            max_docs: 20,
        })
    }

    async fn search_portal(&self, query: &str) -> Result<Vec<PortalHit>, ConnectorError> {
        let resp = self
            .http
            .get(SEARCH_BASE)
            .query(&[("q", query), ("searchDomain", "documents")])
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ConnectorError::Http(format!("{status}: {body}")));
        }

        let html = resp
            .text()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))?;

        Ok(parse_portal_hits(&html))
    }

    async fn fetch_oai_record(&self, slug: &str) -> Result<OaiRecord, ConnectorError> {
        let identifier = oai_identifier(slug);
        let url = format!(
            "{OAI_BASE}?verb=GetRecord&metadataPrefix=oai_dc&identifier={}",
            percent_encode(&identifier)
        );

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ConnectorError::Http(format!("{status}: {body}")));
        }

        let xml = resp
            .text()
            .await
            .map_err(|e| ConnectorError::Parse(e.to_string()))?;

        let records = parse_oai_page(&xml)
            .map_err(|e| ConnectorError::Parse(e.to_string()))?
            .records;

        records
            .into_iter()
            .next()
            .ok_or_else(|| ConnectorError::Parse(format!("Persée GetRecord empty: {identifier}")))
    }
}

#[async_trait]
impl SourceConnector for PerseeConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::Persee
    }

    fn connector_version(&self) -> &str {
        CONNECTOR_VERSION
    }

    async fn discover(
        &self,
        subject: &ResolvedSubject,
        cursor: Option<DiscoveryCursor>,
    ) -> Result<DiscoveryPage, ConnectorError> {
        if cursor.is_some() {
            return Ok(DiscoveryPage {
                documents: vec![],
                next_cursor: None,
            });
        }

        let queries = subject.catalog_query_buckets(SourceKind::Persee);
        let mut documents = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for query in queries {
            let hits = self.search_portal(&query).await?;
            for hit in hits {
                if !seen.insert(hit.slug.clone()) {
                    continue;
                }
                if let Some(doc) = portal_hit_to_discovered(&hit) {
                    documents.push(doc);
                }
                if documents.len() >= self.max_docs as usize {
                    break;
                }
            }
            if documents.len() >= self.max_docs as usize {
                break;
            }
        }

        Ok(DiscoveryPage {
            documents,
            next_cursor: None,
        })
    }

    async fn fetch(
        &self,
        document: &DiscoveredDocument,
    ) -> Result<FetchedDocument, ConnectorError> {
        let slug = document.external_id.trim();
        let record = self.fetch_oai_record(slug).await?;
        let normalized = normalize_persee_record(&record)?;
        let text = normalized.snapshot_text.clone();
        let revision = normalized.revision_token.clone();

        Ok(FetchedDocument {
            discovered: document.clone(),
            revision_id: revision,
            content_type: "text/plain".into(),
            text,
            raw_metadata: serde_json::json!({ "normalized": normalized }),
            license: Some("open access".into()),
            content_bytes: 0,
        })
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        let url = format!("{OAI_BASE}?verb=Identify");
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ConnectorError::Http(e.to_string()))?;
        if resp.status().is_success() {
            Ok(ConnectorHealth {
                ok: true,
                detail: "Persée portal + OAI reachable".into(),
            })
        } else {
            Ok(ConnectorHealth {
                ok: false,
                detail: format!("HTTP {}", resp.status()),
            })
        }
    }
}

fn oai_identifier(slug: &str) -> String {
    format!("oai:persee:article/{slug}")
}

fn portal_hit_to_discovered(hit: &PortalHit) -> Option<DiscoveredDocument> {
    if hit.slug.is_empty() || hit.title.is_empty() {
        return None;
    }

    let year = infer_year_from_slug(&hit.slug);

    Some(DiscoveredDocument {
        source_kind: SourceKind::Persee,
        external_id: hit.slug.clone(),
        canonical_url: Some(format!("https://www.persee.fr/doc/{}", hit.slug)),
        title: hit.title.clone(),
        language: Some("fr".into()),
        document_type: DocumentType::AcademicArticle,
        subject_links: vec![],
        publication_time: year.map(|y| TypedTimeLite::Exact {
            year: y,
            surface: Some(y.to_string()),
        }),
        discovery_method: DiscoveryMethod::CatalogSearch,
        relevance_score: 0.72,
        source_metadata: SourceMetadata {
            raw: serde_json::json!({
                "slug": hit.slug,
                "title": hit.title,
                "oai_identifier": oai_identifier(&hit.slug),
            }),
        },
    })
}

fn infer_year_from_slug(slug: &str) -> Option<i32> {
    slug.split('_')
        .find_map(|part| {
            if part.len() == 4 {
                part.parse::<i32>().ok().filter(|y| (1000..=2100).contains(y))
            } else {
                None
            }
        })
}

pub fn parse_portal_hits(html: &str) -> Vec<PortalHit> {
    let mut hits = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let needle = "https://www.persee.fr/doc/";
    let mut rest = html;
    while let Some(start) = rest.find(needle) {
        let after = &rest[start + needle.len()..];
        let end = after
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
            .unwrap_or(after.len());
        let slug = after[..end].trim().to_string();
        if slug.is_empty() || !seen.insert(slug.clone()) {
            rest = &after[end.min(after.len())..];
            continue;
        }

        let title = extract_title_after(&after[end..]);
        hits.push(PortalHit {
            slug,
            title: title.unwrap_or_else(|| "Sans titre".into()),
        });
        rest = &after[end..];
    }
    hits
}

fn extract_title_after(fragment: &str) -> Option<String> {
    let close = fragment.find('>')?;
    let after_tag = &fragment[close + 1..];
    let end = after_tag.find('<')?;
    let title = after_tag[..end].trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

// ── OAI-PMH XML parsing ──────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct OaiRecord {
    identifier: String,
    title: String,
    creators: Vec<String>,
    subjects: Vec<String>,
    description: Option<String>,
    date: Option<String>,
    publisher: Option<String>,
    source_url: Option<String>,
    language: Option<String>,
}

#[derive(Debug, Default)]
struct OaiPage {
    records: Vec<OaiRecord>,
    resumption_token: Option<String>,
}

fn parse_oai_page(xml: &str) -> Result<OaiPage, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut page = OaiPage::default();
    let mut current: Option<OaiRecord> = None;
    let mut current_field: Option<String> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = std::str::from_utf8(e.local_name().as_ref())
                    .unwrap_or("")
                    .to_string();
                match name.as_str() {
                    "record" => current = Some(OaiRecord::default()),
                    "identifier" if current.is_some() => current_field = Some("identifier".into()),
                    "title" if current.is_some() => current_field = Some("title".into()),
                    "creator" if current.is_some() => current_field = Some("creator".into()),
                    "subject" if current.is_some() => current_field = Some("subject".into()),
                    "description" if current.is_some() => {
                        current_field = Some("description".into())
                    }
                    "date" if current.is_some() => current_field = Some("date".into()),
                    "publisher" if current.is_some() => current_field = Some("publisher".into()),
                    "source" if current.is_some() => current_field = Some("source".into()),
                    "language" if current.is_some() => current_field = Some("language".into()),
                    "resumptionToken" => current_field = Some("resumptionToken".into()),
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let name = std::str::from_utf8(e.local_name().as_ref())
                    .unwrap_or("")
                    .to_string();
                if name == "record" {
                    if let Some(rec) = current.take() {
                        if !rec.identifier.is_empty() && !rec.title.is_empty() {
                            page.records.push(rec);
                        }
                    }
                }
                current_field = None;
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().trim().to_string();
                if text.is_empty() {
                    continue;
                }
                match current_field.as_deref() {
                    Some("resumptionToken") => {
                        if !text.is_empty() {
                            page.resumption_token = Some(text);
                        }
                    }
                    Some(field) => {
                        if let Some(rec) = current.as_mut() {
                            match field {
                                "identifier" if rec.identifier.is_empty() => {
                                    rec.identifier = text;
                                }
                                "title" if rec.title.is_empty() => {
                                    rec.title = text;
                                }
                                "creator" => rec.creators.push(text),
                                "subject" => rec.subjects.push(text),
                                "description" if rec.description.is_none() => {
                                    rec.description = Some(text);
                                }
                                "date" if rec.date.is_none() => {
                                    rec.date = Some(text);
                                }
                                "publisher" if rec.publisher.is_none() => {
                                    rec.publisher = Some(text);
                                }
                                "source" if rec.source_url.is_none() => {
                                    if text.starts_with("http") {
                                        rec.source_url = Some(text);
                                    }
                                }
                                "language" if rec.language.is_none() => {
                                    rec.language = Some(text);
                                }
                                _ => {}
                            }
                        }
                    }
                    None => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.to_string()),
            _ => {}
        }
        buf.clear();
    }

    Ok(page)
}

pub fn normalize_persee_record(
    record: &OaiRecord,
) -> Result<NormalizedCorpusDocument, ConnectorError> {
    if record.title.is_empty() {
        return Err(ConnectorError::Parse("Persée record missing title".into()));
    }
    if record.identifier.is_empty() {
        return Err(ConnectorError::Parse("Persée record missing identifier".into()));
    }

    let year = record
        .date
        .as_deref()
        .and_then(|d| d.get(..4))
        .and_then(|y| y.parse::<i32>().ok());

    let publication_time = year
        .map(|y| TypedTimeLite::Exact {
            year: y,
            surface: Some(y.to_string()),
        })
        .unwrap_or(TypedTimeLite::Unknown { surface: None });

    let canonical_url = record.source_url.clone().or_else(|| {
        record
            .identifier
            .strip_prefix("oai:persee:article/")
            .map(|slug| format!("https://www.persee.fr/doc/{slug}"))
    });

    let subjects: Vec<NormalizedSubject> = record
        .subjects
        .iter()
        .map(|s| NormalizedSubject {
            scheme: "keyword".into(),
            label: s.clone(),
            identifier: None,
        })
        .collect();

    let contributions = record
        .creators
        .iter()
        .enumerate()
        .map(|(i, name)| crate::corpus::NormalizedContribution {
            role: ContributionRole::Author,
            agent_name: name.clone(),
            name_normalized: name.to_lowercase(),
            identifier_scheme: None,
            identifier_value: None,
            ordinal: i as i32,
        })
        .collect();

    let snapshot_text = {
        let mut parts = vec![format!("TITLE: {}", record.title)];
        if let Some(desc) = &record.description {
            parts.push(format!("ABSTRACT: {desc}"));
        }
        if !subjects.is_empty() {
            let kws = subjects
                .iter()
                .map(|s| s.label.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            parts.push(format!("KEYWORDS: {kws}"));
        }
        parts.join("\n")
    };

    let mut doc = NormalizedCorpusDocument {
        source_kind: SourceKind::Persee,
        external_id: record.identifier.clone(),
        canonical_url,
        document_type: DocumentType::AcademicArticle,
        title: record.title.clone(),
        language: record.language.clone(),
        abstract_text: record.description.clone(),
        academic_status: AcademicStatus::PeerReviewed,
        access_level: AccessLevel::Open,
        full_text_available: false,
        rights_uri: Some("https://www.persee.fr/apropos/conditions-utilisation".into()),
        rights_holder: Some("Persée / CNRS".into()),
        rights_normalized: AccessLevel::Open,
        publisher_or_institution: record.publisher.clone(),
        publication_time,
        identifiers: vec![],
        contributions,
        subjects,
        connector_version: CONNECTOR_VERSION.into(),
        snapshot_text,
        revision_token: None,
        raw_metadata: serde_json::json!({
            "identifier": record.identifier,
            "title": record.title,
        }),
    };
    let fp = doc.content_fingerprint();
    doc.revision_token = Some(fp);
    Ok(doc)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_portal_hits_extracts_slug_and_title() {
        let html = r#"<a href="https://www.persee.fr/doc/jds_0021-8103_1911_num_9_8_3761">Christophe Colomb</a>"#;
        let hits = parse_portal_hits(html);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].slug, "jds_0021-8103_1911_num_9_8_3761");
        assert_eq!(hits[0].title, "Christophe Colomb");
    }

    #[test]
    fn infer_year_from_slug_reads_publication_year() {
        assert_eq!(
            infer_year_from_slug("jds_0021-8103_1911_num_9_8_3761"),
            Some(1911)
        );
    }
}
