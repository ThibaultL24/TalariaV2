// crates/talaria-sources/src/connectors/commons.rs
use async_trait::async_trait;
use serde_json::Value;

use crate::connector::{
    ConnectorError, ConnectorHealth, DiscoveryCursor, DiscoveryPage, FetchedDocument,
    SourceConnector,
};
use crate::connectors::catalog::{
    bibliographic_notice, catalog_place, http_client, parse_year, year_in_life, NoticeRelation,
};
use crate::kinds::{DiscoveryMethod, DocumentType, SourceKind};
use crate::plan::ResolvedSubject;
use crate::types::{DiscoveredDocument, ExternalEntityRef, SourceMetadata, TypedTimeLite};

const API: &str = "https://commons.wikimedia.org/w/api.php";
const FILE_NS: i64 = 6;

pub struct WikimediaCommonsConnector {
    http: reqwest::Client,
    max_docs: u32,
}

impl WikimediaCommonsConnector {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            http: http_client()?,
            max_docs: 10,
        })
    }

    pub fn parse_generator_pages(
        subject: &ResolvedSubject,
        payload: &Value,
    ) -> Vec<DiscoveredDocument> {
        let Some(pages) = payload.pointer("/query/pages").and_then(|v| v.as_object()) else {
            return vec![];
        };
        let mut out = Vec::new();
        for page in pages.values() {
            let Some(title) = page.get("title").and_then(|t| t.as_str()) else {
                continue;
            };
            let ns = page.get("ns").and_then(|n| n.as_i64()).unwrap_or(FILE_NS);
            if ns != FILE_NS && !title.starts_with("File:") && !title.starts_with("Fichier:") {
                continue;
            }
            let info = page
                .get("imageinfo")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first());
            let ext = info
                .and_then(|i| i.get("extmetadata"))
                .cloned()
                .unwrap_or(Value::Null);
            let description = ext_value(&ext, "ImageDescription")
                .or_else(|| ext_value(&ext, "ObjectName"))
                .map(|s| strip_markup(&s));
            let date_raw =
                ext_value(&ext, "DateTimeOriginal").or_else(|| ext_value(&ext, "DateTime"));
            let year = date_raw.as_deref().and_then(parse_year);
            if let Some(year) = year {
                if !year_in_life(year, subject) {
                    continue;
                }
            }
            let coords = page
                .get("coordinates")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first());
            let lat = coords.and_then(|c| c.get("lat")).and_then(|v| v.as_f64());
            let lon = coords.and_then(|c| c.get("lon")).and_then(|v| v.as_f64());
            let place =
                catalog_place(description.as_deref()).or_else(|| catalog_place(Some(title)));
            let url = info
                .and_then(|i| i.get("descriptionurl").or_else(|| i.get("url")))
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| {
                    format!(
                        "https://commons.wikimedia.org/wiki/{}",
                        title.replace(' ', "_")
                    )
                });
            let notice = caption_notice(
                &subject.label,
                title,
                year,
                place.as_deref(),
                description.as_deref(),
                lat,
                lon,
            );
            if notice.trim().is_empty() {
                continue;
            }
            let mentions_subject = crate::connectors::catalog::names_match(&subject.label, title)
                || description
                    .as_deref()
                    .is_some_and(|d| crate::connectors::catalog::names_match(&subject.label, d));
            if !mentions_subject {
                continue;
            }
            let score = if lat.is_some() && lon.is_some() && year.is_some() {
                0.86
            } else if year.is_some() && place.is_some() {
                0.78
            } else {
                0.58
            };
            out.push(DiscoveredDocument {
                source_kind: SourceKind::WikimediaCommons,
                external_id: title.to_string(),
                canonical_url: Some(url),
                title: title.to_string(),
                language: Some("en".into()),
                document_type: DocumentType::MediaCaption,
                subject_links: vec![ExternalEntityRef {
                    system: "commons".into(),
                    id: title.to_string(),
                    label: Some(subject.label.clone()),
                }],
                publication_time: year.map(|y| TypedTimeLite::Exact {
                    year: y,
                    surface: Some(y.to_string()),
                }),
                discovery_method: DiscoveryMethod::CatalogSearch,
                relevance_score: score,
                source_metadata: SourceMetadata {
                    raw: serde_json::json!({
                        "notice": notice,
                        "place": place,
                        "lat": lat,
                        "lon": lon,
                        "year": year,
                    }),
                },
            });
        }
        out
    }
}

fn ext_value(ext: &Value, key: &str) -> Option<String> {
    ext.get(key)
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty())
}

fn strip_markup(raw: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in raw.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn caption_notice(
    subject: &str,
    title: &str,
    year: Option<i32>,
    place: Option<&str>,
    description: Option<&str>,
    lat: Option<f64>,
    lon: Option<f64>,
) -> String {
    let mut lines = Vec::new();
    let clean_title = title
        .trim_start_matches("File:")
        .trim_start_matches("Fichier:")
        .trim();
    lines.push(bibliographic_notice(
        subject,
        clean_title,
        year,
        place,
        description,
        NoticeRelation::About,
    ));
    if let (Some(year), Some(place)) = (year, place) {
        lines.push(format!("{subject} visited {place} in {year}."));
    }
    if let (Some(year), Some(place), Some(lat), Some(lon)) = (year, place, lat, lon) {
        lines.push(format!(
            "STATEMENT\tvisited\tdepicted_at\t{year}\t{place}\t\t{lat}\t{lon}"
        ));
    }
    lines.join("\n")
}

#[async_trait]
impl SourceConnector for WikimediaCommonsConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::WikimediaCommons
    }

    fn connector_version(&self) -> &str {
        "commons:imageinfo_v1"
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
        let query = subject.catalog_query(SourceKind::WikimediaCommons);
        let response = self
            .http
            .get(API)
            .query(&[
                ("action", "query"),
                ("generator", "search"),
                ("gsrsearch", query.as_str()),
                ("gsrnamespace", "6"),
                ("gsrlimit", &self.max_docs.to_string()),
                ("prop", "imageinfo|coordinates"),
                ("iiprop", "url|extmetadata|canonicaltitle"),
                (
                    "iiextmetadatafilter",
                    "ImageDescription|DateTimeOriginal|DateTime|ObjectName|LicenseShortName",
                ),
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
        let mut docs = Self::parse_generator_pages(subject, &response);
        docs.truncate(self.max_docs as usize);
        Ok(DiscoveryPage {
            documents: docs,
            next_cursor: None,
        })
    }

    async fn fetch(
        &self,
        document: &DiscoveredDocument,
    ) -> Result<FetchedDocument, ConnectorError> {
        let text = document
            .source_metadata
            .raw
            .get("notice")
            .and_then(|v| v.as_str())
            .unwrap_or(&document.title)
            .to_string();
        let bytes = text.len() as u64;
        Ok(FetchedDocument {
            discovered: document.clone(),
            revision_id: None,
            content_type: "text/plain".into(),
            text,
            raw_metadata: document.source_metadata.raw.clone(),
            license: Some("Wikimedia Commons (file license in metadata)".into()),
            content_bytes: bytes,
        })
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        Ok(ConnectorHealth {
            ok: true,
            detail: "commons.wikimedia.org imageinfo".into(),
        })
    }
}
