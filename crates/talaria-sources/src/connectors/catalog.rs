// crates/talaria-sources/src/connectors/catalog.rs
//! Shared bibliographic notice text for catalog connectors.

use serde_json::Value;

use crate::plan::ResolvedSubject;

pub const UA: &str = "TalariaEngine/0.1 (https://github.com/talaria; catalog ingest)";

pub fn http_client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder().user_agent(UA).build()?)
}

#[derive(Debug, Clone, Copy)]
pub enum NoticeRelation {
    Authored,
    About,
}

/// Turn catalog metadata into extractor-friendly prose. Does not invent verbs
/// beyond "published" when the subject is the recorded author.
pub fn bibliographic_notice(
    subject: &str,
    title: &str,
    year: Option<i32>,
    place: Option<&str>,
    description: Option<&str>,
    relation: NoticeRelation,
) -> String {
    let mut lines = Vec::new();
    match (relation, year, place) {
        (NoticeRelation::Authored, Some(year), Some(place)) if !place.is_empty() => {
            lines.push(format!(
                "{subject} published \"{title}\" in {year} in {place}."
            ));
        }
        (NoticeRelation::Authored, Some(year), _) => {
            lines.push(format!("{subject} published \"{title}\" in {year}."));
        }
        _ => {
            lines.push(title.to_string());
        }
    }
    if let Some(desc) = description {
        let trimmed = desc.trim();
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
    }
    lines.join("\n")
}

pub fn folded(s: &str) -> String {
    s.to_lowercase()
        .replace(['à', 'á', 'â', 'ä'], "a")
        .replace(['è', 'é', 'ê', 'ë'], "e")
        .replace(['ì', 'í', 'î', 'ï'], "i")
        .replace(['ò', 'ó', 'ô', 'ö'], "o")
        .replace(['ù', 'ú', 'û', 'ü'], "u")
        .replace('ç', "c")
        .replace('ñ', "n")
}

pub fn names_match(person: &str, candidate: &str) -> bool {
    let person = folded(person);
    let candidate = folded(candidate);
    !person.is_empty() && (candidate.contains(&person) || person.contains(&candidate))
}

pub fn year_in_life(year: i32, subject: &ResolvedSubject) -> bool {
    match (subject.birth_year, subject.death_year) {
        (Some(birth), Some(death)) => (birth.saturating_sub(5)..=death.saturating_add(2)).contains(&year),
        (Some(birth), None) => year >= birth.saturating_sub(5),
        (None, Some(death)) => year <= death.saturating_add(2),
        (None, None) => (1000..=2100).contains(&year),
    }
}

pub fn json_first_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Array(items) => items.iter().find_map(json_first_string),
        Value::Object(map) => map
            .get("def")
            .or_else(|| map.get("en"))
            .or_else(|| map.values().next())
            .and_then(json_first_string),
        _ => None,
    }
}

pub fn parse_year(raw: &str) -> Option<i32> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).take(4).collect();
    if digits.len() != 4 {
        return None;
    }
    let year: i32 = digits.parse().ok()?;
    (1000..=2100).contains(&year).then_some(year)
}

pub fn xml_texts(hay: &str, tag: &str) -> Vec<String> {
    let open_start = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut rest = hay;
    while let Some(start) = rest.find(&open_start) {
        let after_name = &rest[start + open_start.len()..];
        let Some(gt) = after_name.find('>') else {
            break;
        };
        let after = &after_name[gt + 1..];
        let Some(end) = after.find(&close) else {
            break;
        };
        let text = after[..end]
            .replace("&quot;", "\"")
            .replace("&amp;", "&")
            .replace("&apos;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .trim()
            .to_string();
        if !text.is_empty() {
            out.push(text);
        }
        rest = &after[end + close.len()..];
    }
    out
}
