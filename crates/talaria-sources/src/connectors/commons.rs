// crates/talaria-sources/src/connectors/commons.rs
use serde_json::Value;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
