// crates/talaria-sources/src/connectors/commons.rs
use std::collections::HashSet;

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

/// Parsed Commons MediaInfo + imageinfo metadata (no binary fetch).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonsAsset {
    pub commons_file: String,
    pub mid: Option<String>,
    pub sha1: Option<String>,
    pub mime: Option<String>,
    pub license: Option<String>,
    pub attribution_text: String,
    pub thumb_url: Option<String>,
    pub depicts_qids: Vec<String>,
    pub revision_id: Option<String>,
    pub rights_normalized: String, // open | restricted | metadata_only | unknown
}

/// Parse a MediaInfo entity plus optional imageinfo array into a Commons asset.
/// Returns `None` when attribution text would be empty.
pub fn parse_mediainfo(entity: &Value, imageinfo: Option<&Value>) -> Option<CommonsAsset> {
    let info = imageinfo.and_then(|v| v.as_array()).and_then(|a| a.first());

    let license = info
        .and_then(|i| i.pointer("/extmetadata/LicenseShortName/value"))
        .and_then(Value::as_str)
        .map(strip_html_tags)
        .filter(|s| !s.is_empty());

    let attribution_text = attribution_from_entity(entity).or_else(|| {
        info.and_then(|i| {
            let artist = extmetadata_value(i, "Artist")
                .or_else(|| extmetadata_value(i, "Author"));
            let license_label = extmetadata_value(i, "LicenseShortName");
            build_attribution(artist.as_deref(), license_label.as_deref())
        })
    })?;

    if attribution_text.trim().is_empty() {
        return None;
    }

    let entity_id = entity.get("id").and_then(Value::as_str);
    let mid = entity_id.filter(|id| id.starts_with('M')).map(str::to_string);

    let commons_file = entity
        .get("title")
        .and_then(Value::as_str)
        .map(strip_html_tags)
        .filter(|s| !s.is_empty())
        .or_else(|| info.and_then(filename_from_descriptionurl))
        .or_else(|| entity_id.map(str::to_string))
        .unwrap_or_default();

    let depicts_qids = depicts_from_entity(entity);
    let rights_normalized = normalize_rights(license.as_deref());

    Some(CommonsAsset {
        commons_file,
        mid,
        sha1: info
            .and_then(|i| i.get("sha1"))
            .and_then(Value::as_str)
            .map(str::to_string),
        mime: info
            .and_then(|i| i.get("mime"))
            .and_then(Value::as_str)
            .map(str::to_string),
        license,
        attribution_text,
        thumb_url: info
            .and_then(|i| i.get("thumburl"))
            .and_then(Value::as_str)
            .map(str::to_string),
        depicts_qids,
        revision_id: info
            .and_then(|i| i.get("revid"))
            .and_then(|v| v.as_i64().map(|n| n.to_string()).or_else(|| v.as_str().map(str::to_string))),
        rights_normalized,
    })
}

fn strip_html_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extmetadata_value(info: &Value, key: &str) -> Option<String> {
    let raw = info
        .pointer(&format!("/extmetadata/{key}/value"))
        .and_then(Value::as_str)?;
    let cleaned = strip_html_tags(raw);
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn build_attribution(creator: Option<&str>, license: Option<&str>) -> Option<String> {
    match (creator, license) {
        (Some(c), Some(l)) => Some(format!("{c} — {l}")),
        (Some(c), None) => Some(c.to_string()),
        (None, Some(l)) => Some(l.to_string()),
        (None, None) => None,
    }
}

fn attribution_from_entity(entity: &Value) -> Option<String> {
    let creator = statement_labels(entity, "P2091")
        .into_iter()
        .chain(statement_labels(entity, "P170"))
        .next();
    creator.map(|c| c)
}

fn statement_labels(entity: &Value, property: &str) -> Vec<String> {
    let Some(statements) = entity
        .pointer("/statements")
        .and_then(Value::as_object)
        .and_then(|s| s.get(property))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };

    statements
        .iter()
        .filter_map(|claim| snak_label(claim.pointer("/mainsnak")))
        .collect()
}

fn snak_label(snak: Option<&Value>) -> Option<String> {
    let snak = snak?;
    if snak.get("snaktype").and_then(Value::as_str) != Some("value") {
        return None;
    }
    let datavalue = snak.get("datavalue")?;
    match datavalue.get("type").and_then(Value::as_str)? {
        "string" => datavalue
            .get("value")
            .and_then(Value::as_str)
            .map(strip_html_tags)
            .filter(|s| !s.is_empty()),
        "monolingualtext" => datavalue
            .pointer("/value/text")
            .and_then(Value::as_str)
            .map(strip_html_tags)
            .filter(|s| !s.is_empty()),
        "wikibase-entityid" => entity_value_label(datavalue.get("value")),
        _ => None,
    }
}

/// Human-readable label from an expanded entity-id value; QIDs alone are not attribution.
fn entity_value_label(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        let cleaned = strip_html_tags(text);
        if !cleaned.is_empty() {
            return Some(cleaned);
        }
    }
    label_from_labels_map(value.get("labels"))
}

fn label_from_labels_map(labels: Option<&Value>) -> Option<String> {
    let labels = labels?.as_object()?;
    labels
        .get("en")
        .or_else(|| labels.values().next())
        .and_then(|entry| entry.get("value").and_then(Value::as_str))
        .map(strip_html_tags)
        .filter(|s| !s.is_empty())
}

fn depicts_from_entity(entity: &Value) -> Vec<String> {
    entity
        .pointer("/statements/P180")
        .and_then(Value::as_array)
        .map(|claims| {
            claims
                .iter()
                .filter_map(|claim| {
                    claim
                        .pointer("/mainsnak/datavalue/value/id")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn filename_from_descriptionurl(info: &Value) -> Option<String> {
    let url = info.get("descriptionurl").and_then(Value::as_str)?;
    url.rsplit('/').next().filter(|s| !s.is_empty()).map(str::to_string)
}

fn normalize_rights(license: Option<&str>) -> String {
    let Some(license) = license else {
        return "unknown".into();
    };
    let upper = license.to_ascii_uppercase();
    if upper.contains("CC BY")
        || upper.contains("CC0")
        || upper.contains("PUBLIC DOMAIN")
        || upper.contains(" PD")
        || upper.starts_with("PD")
    {
        "open".into()
    } else {
        "restricted".into()
    }
}

const API: &str = "https://commons.wikimedia.org/w/api.php";
const CONNECTOR_VERSION: &str = "commons:mediainfo_v1";
const UNLICENSED: &str = "unlicensed or missing attribution";
const ID_SYSTEMS: &[&str] = &["commons", "commonswiki", "p18", "p1442", "p109"];

/// Commons MediaInfo connector (metadata + thumb URL; never original bytes).
pub struct CommonsConnector {
    pub http: reqwest::Client,
    pub max_docs: u32,
}

impl CommonsConnector {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            http: http_client()?,
            max_docs: 10,
        })
    }

    fn cap(&self) -> usize {
        self.max_docs.min(10) as usize
    }

    pub fn document_from_p18(title: &str) -> DiscoveredDocument {
        let file_title = ensure_file_title(title);
        let canonical = format!(
            "https://commons.wikimedia.org/wiki/{}",
            file_title.replace(' ', "_")
        );
        DiscoveredDocument {
            source_kind: SourceKind::WikimediaCommons,
            external_id: file_title.clone(),
            canonical_url: Some(canonical),
            title: file_title.clone(),
            language: None,
            document_type: DocumentType::MediaCaption,
            subject_links: vec![ExternalEntityRef {
                system: "commons".into(),
                id: file_title.clone(),
                label: Some(file_title),
            }],
            publication_time: None,
            discovery_method: DiscoveryMethod::IdentifierLookup,
            relevance_score: 0.9,
            source_metadata: SourceMetadata::default(),
        }
    }

    async fn fetch_wbgetentities(&self, mid: &str) -> Result<Value, ConnectorError> {
        self.http
            .get(API)
            .query(&[
                ("action", "wbgetentities"),
                ("ids", mid),
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

    async fn fetch_imageinfo(&self, title: &str) -> Result<Value, ConnectorError> {
        self.http
            .get(API)
            .query(&[
                ("action", "query"),
                ("titles", title),
                ("prop", "imageinfo|revisions"),
                ("iiprop", "url|size|mime|sha1|extmetadata"),
                ("iiurlwidth", "640"),
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

/// Filenames from Wikidata P18 / P1442 / P109 mainsnak strings or File-titled items.
pub fn parse_p18_filenames(claims: &Value) -> Vec<String> {
    let obj = claims.get("claims").unwrap_or(claims);
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for pid in ["P18", "P1442", "P109"] {
        let Some(arr) = obj.get(pid).and_then(Value::as_array) else {
            continue;
        };
        for stmt in arr {
            if let Some(name) = filename_from_mainsnak(stmt.get("mainsnak")) {
                if seen.insert(name.clone()) {
                    out.push(name);
                }
            }
        }
    }
    out
}

fn filename_from_mainsnak(snak: Option<&Value>) -> Option<String> {
    let snak = snak?;
    if snak.get("snaktype").and_then(Value::as_str) != Some("value") {
        return None;
    }
    let dv = snak.get("datavalue")?;
    if let Some(s) = dv.get("value").and_then(Value::as_str) {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    let value = dv.get("value")?;
    if let Some(title) = value.get("title").and_then(Value::as_str) {
        if title.len() >= 5 && title[..5].eq_ignore_ascii_case("file:") {
            return Some(title.to_string());
        }
    }
    if let Some(id) = value.get("id").and_then(Value::as_str) {
        if id.len() >= 5 && id[..5].eq_ignore_ascii_case("file:") {
            return Some(id.to_string());
        }
    }
    None
}

fn is_mid(s: &str) -> bool {
    let s = s.trim();
    s.len() >= 2 && s.as_bytes()[0] == b'M' && s[1..].chars().all(|c| c.is_ascii_digit())
}

fn ensure_file_title(title: &str) -> String {
    let t = title.trim();
    if is_mid(t) {
        return t.to_string();
    }
    if t.len() >= 5 && t[..5].eq_ignore_ascii_case("file:") {
        format!("File:{}", &t[5..])
    } else {
        format!("File:{t}")
    }
}

fn is_commons_system(system: &str) -> bool {
    ID_SYSTEMS.iter().any(|s| system.eq_ignore_ascii_case(s))
}

fn strip_original_url(imageinfo: &mut Value) {
    if let Some(arr) = imageinfo.as_array_mut() {
        for item in arr {
            if let Some(obj) = item.as_object_mut() {
                obj.remove("url");
            }
        }
    } else if let Some(obj) = imageinfo.as_object_mut() {
        obj.remove("url");
    }
}

fn entity_from_wbgetentities(json: &Value, mid: &str) -> Option<Value> {
    json.pointer(&format!("/entities/{mid}")).cloned()
}

fn page_from_query(json: &Value) -> Option<&Value> {
    json.pointer("/query/pages")
        .and_then(Value::as_object)
        .and_then(|pages| pages.values().next())
}

fn fetched_from_asset(
    document: &DiscoveredDocument,
    entity: Value,
    mut imageinfo: Value,
    asset: &CommonsAsset,
) -> FetchedDocument {
    strip_original_url(&mut imageinfo);
    let raw_metadata = serde_json::json!({
        "entity": entity,
        "imageinfo": imageinfo,
        "thumburl": asset.thumb_url,
        "commons_file": asset.commons_file,
        "mid": asset.mid,
    });
    FetchedDocument {
        discovered: document.clone(),
        revision_id: asset.revision_id.clone(),
        content_type: "application/json".into(),
        text: asset.attribution_text.clone(),
        raw_metadata,
        license: asset.license.clone(),
        content_bytes: asset.attribution_text.len() as u64,
    }
}

#[async_trait]
impl SourceConnector for CommonsConnector {
    fn source_kind(&self) -> SourceKind {
        SourceKind::WikimediaCommons
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

        let mut titles = Vec::new();
        let mut seen = HashSet::new();
        for (system, id) in &subject.known_identifiers {
            if !is_commons_system(system) {
                continue;
            }
            let id = id.trim();
            if id.is_empty() || !seen.insert(id.to_string()) {
                continue;
            }
            titles.push(id.to_string());
        }
        titles.truncate(self.cap());
        let documents = titles
            .into_iter()
            .map(|t| Self::document_from_p18(&t))
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
        let (entity, imageinfo) = if is_mid(&document.external_id) {
            let json = self.fetch_wbgetentities(&document.external_id).await?;
            let entity = entity_from_wbgetentities(&json, &document.external_id)
                .ok_or_else(|| ConnectorError::Parse(UNLICENSED.into()))?;
            (entity, Value::Null)
        } else {
            let title = ensure_file_title(&document.external_id);
            let json = self.fetch_imageinfo(&title).await?;
            let page =
                page_from_query(&json).ok_or_else(|| ConnectorError::Parse(UNLICENSED.into()))?;
            let imageinfo = page.get("imageinfo").cloned().unwrap_or(Value::Null);
            let mid = page
                .pointer("/pageprops/wikibase_item")
                .and_then(Value::as_str);
            let entity = serde_json::json!({
                "id": mid,
                "title": page.get("title").and_then(Value::as_str).unwrap_or(&title),
            });
            (entity, imageinfo)
        };

        let imageinfo_ref = if imageinfo.is_null() {
            None
        } else {
            Some(&imageinfo)
        };
        let asset = parse_mediainfo(&entity, imageinfo_ref)
            .ok_or_else(|| ConnectorError::Parse(UNLICENSED.into()))?;
        Ok(fetched_from_asset(document, entity, imageinfo, &asset))
    }

    async fn healthcheck(&self) -> Result<ConnectorHealth, ConnectorError> {
        Ok(ConnectorHealth {
            ok: true,
            detail: "commons.wikimedia.org Action API".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connector::SourceConnector;
    use crate::kinds::{DocumentType, SourceKind};
    use crate::plan::ResolvedSubject;

    #[test]
    fn mediainfo_requires_attribution() {
        let entity = serde_json::json!({"id":"M1","statements":{}});
        let info = serde_json::json!([{"thumburl":"https://upload.wikimedia.org/x.jpg","mime":"image/jpeg"}]);
        assert!(parse_mediainfo(&entity, Some(&info)).is_none());
    }

    #[test]
    fn mediainfo_ok() {
        let entity = serde_json::json!({"id":"M123","statements":{
            "P180":[{"mainsnak":{"datavalue":{"value":{"id":"Q517"}}}}]
        }});
        let info = serde_json::json!([{
            "thumburl":"https://upload.wikimedia.org/thumb/x.jpg",
            "mime":"image/jpeg",
            "sha1":"abc",
            "extmetadata":{
                "Artist":{"value":"Louvre"},
                "LicenseShortName":{"value":"CC BY-SA 4.0"}
            }
        }]);
        let a = parse_mediainfo(&entity, Some(&info)).unwrap();
        assert!(a.attribution_text.contains("Louvre"));
        assert_eq!(a.depicts_qids, ["Q517"]);
        assert_eq!(a.rights_normalized, "open");
    }

    #[test]
    fn mediainfo_p170_qid_without_label_falls_back_to_extmetadata() {
        let entity = serde_json::json!({"id":"M456","statements":{
            "P170":[{"mainsnak":{"snaktype":"value","property":"P170","datavalue":{
                "type":"wikibase-entityid",
                "value":{"entity-type":"item","numeric-id":1,"id":"Q1"}
            }}}]
        }});
        let info = serde_json::json!([{
            "thumburl":"https://upload.wikimedia.org/thumb/y.jpg",
            "mime":"image/jpeg",
            "extmetadata":{
                "Artist":{"value":"Louvre"},
                "LicenseShortName":{"value":"CC BY-SA 4.0"}
            }
        }]);
        let a = parse_mediainfo(&entity, Some(&info)).unwrap();
        assert!(a.attribution_text.contains("Louvre"));
        assert!(!a.attribution_text.contains("Q1"));
    }

    #[test]
    fn document_from_p18_normalizes_file_prefix_and_canonical_url() {
        let d = CommonsConnector::document_from_p18("File:x.jpg");
        assert_eq!(d.source_kind, SourceKind::WikimediaCommons);
        assert_eq!(d.document_type, DocumentType::MediaCaption);
        assert_eq!(
            d.canonical_url.as_deref(),
            Some("https://commons.wikimedia.org/wiki/File:x.jpg")
        );
        let bare = CommonsConnector::document_from_p18("x.jpg");
        assert_eq!(
            bare.canonical_url.as_deref(),
            Some("https://commons.wikimedia.org/wiki/File:x.jpg")
        );
    }

    fn subject_with_ids(ids: Vec<(String, String)>) -> ResolvedSubject {
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
    async fn discover_empty_without_identifiers() {
        let conn = CommonsConnector::new().unwrap();
        let page = conn.discover(&subject_with_ids(vec![]), None).await.unwrap();
        assert!(page.documents.is_empty());
    }

    #[tokio::test]
    async fn discover_from_known_identifiers() {
        let conn = CommonsConnector::new().unwrap();
        let page = conn
            .discover(
                &subject_with_ids(vec![
                    ("wikidata".into(), "Q517".into()),
                    ("commons".into(), "File:Napoleon.jpg".into()),
                    ("P18".into(), "Portrait.png".into()),
                    ("commonswiki".into(), "File:Signature.svg".into()),
                ]),
                None,
            )
            .await
            .unwrap();
        assert_eq!(page.documents.len(), 3);
        assert!(page
            .documents
            .iter()
            .all(|d| d.source_kind == SourceKind::WikimediaCommons));
        assert!(page
            .documents
            .iter()
            .all(|d| d.document_type == DocumentType::MediaCaption));
    }

    #[test]
    fn parse_p18_filenames_from_fixture_claims() {
        let claims = serde_json::json!({
            "P18": [{"mainsnak": {"snaktype": "value", "datavalue": {
                "type": "string", "value": "Napoleon Bonaparte.jpg"
            }}}],
            "P1442": [{"mainsnak": {"snaktype": "value", "datavalue": {
                "type": "string", "value": "File:Napoleon grave.jpg"
            }}}],
            "P109": [{"mainsnak": {"snaktype": "value", "datavalue": {
                "type": "wikibase-entityid",
                "value": {"id": "Q1", "title": "File:Napoleon Signature.svg"}
            }}}],
            "P31": [{"mainsnak": {"snaktype": "value", "datavalue": {
                "type": "string", "value": "not-an-image.jpg"
            }}}]
        });
        let files = parse_p18_filenames(&claims);
        assert_eq!(
            files,
            vec![
                "Napoleon Bonaparte.jpg".to_string(),
                "File:Napoleon grave.jpg".to_string(),
                "File:Napoleon Signature.svg".to_string(),
            ]
        );
    }

    #[test]
    fn live_registry_marks_commons_implemented() {
        let reg = crate::connectors::default_registry(None, true).unwrap();
        let entry = reg
            .get(&SourceKind::WikimediaCommons)
            .expect("commons registered");
        assert!(entry.implemented);
    }
}
