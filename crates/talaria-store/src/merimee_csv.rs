// crates/talaria-store/src/merimee_csv.rs
//! Streaming parser for the official data.gouv.fr Mérimée CSV (semicolon-separated).

use crate::visit_catalog::person_named_in;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;

pub const DATASET_URI: &str =
    "https://www.data.gouv.fr/datasets/immeubles-proteges-au-titre-des-monuments-historiques-3";
pub const DEFAULT_CSV_URL: &str =
    "https://static.data.gouv.fr/resources/immeubles-proteges-au-titre-des-monuments-historiques-3/20240213-133323/liste-des-immeubles-proteges-au-titre-des-monuments-historiques.csv";

#[derive(Debug, Clone, PartialEq)]
pub struct MerimeePersonEntry {
    pub raw: String,
    pub name_normalized: String,
    pub role_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedMerimeeRow {
    pub reference: String,
    pub title: String,
    pub commune: Option<String>,
    pub department: Option<String>,
    pub region: Option<String>,
    pub protection_kind: Option<String>,
    pub authors: Option<String>,
    pub historique: Option<String>,
    pub description: Option<String>,
    pub personalities_raw: Option<String>,
    pub persons: Vec<MerimeePersonEntry>,
    pub external_links_raw: Option<String>,
    pub place_wikidata_qid: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub content_hash: String,
    pub row_number: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowReject {
    MissingReference,
    InvalidReference,
    MissingTitle,
    InvalidCoordinates,
    MalformedRow(String),
}

pub fn notice_content_hash(
    reference: &str,
    title: &str,
    commune: Option<&str>,
    lat: Option<f64>,
    lon: Option<f64>,
    personalities_raw: Option<&str>,
    protection_kind: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(reference.trim().as_bytes());
    hasher.update(b"|");
    hasher.update(title.trim().as_bytes());
    hasher.update(b"|");
    hasher.update(commune.unwrap_or("").trim().as_bytes());
    hasher.update(b"|");
    if let (Some(lat), Some(lon)) = (lat, lon) {
        hasher.update(format!("{lat:.6}|{lon:.6}").as_bytes());
    }
    hasher.update(b"|");
    hasher.update(personalities_raw.unwrap_or("").trim().as_bytes());
    hasher.update(b"|");
    hasher.update(protection_kind.unwrap_or("").trim().as_bytes());
    hex::encode(hasher.finalize())
}

pub fn parse_personalities_field(raw: &str) -> Vec<MerimeePersonEntry> {
    let mut out = Vec::new();
    for segment in split_person_segments(raw) {
        let trimmed = segment.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (name, role) = split_name_role(trimmed);
        let normalized = normalize_person_name(&name);
        if normalized.is_empty() {
            continue;
        }
        out.push(MerimeePersonEntry {
            raw: trimmed.to_string(),
            name_normalized: normalized,
            role_label: role,
        });
    }
    out
}

fn split_person_segments(raw: &str) -> Vec<String> {
    raw.split(['\n', '|'])
        .flat_map(|line| line.split(';'))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn split_name_role(value: &str) -> (String, Option<String>) {
    let Some(open) = value.rfind('(') else {
        return (value.to_string(), None);
    };
    let Some(close) = value.rfind(')') else {
        return (value.to_string(), None);
    };
    if close < open {
        return (value.to_string(), None);
    }
    let name = value[..open].trim();
    let role = value[open + 1..close].trim();
    if name.is_empty() {
        return (value.to_string(), None);
    }
    (
        name.to_string(),
        if role.is_empty() { None } else { Some(role.to_string()) },
    )
}

pub fn normalize_person_name(name: &str) -> String {
    fold(name)
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn extract_wikidata_qid(links: &str) -> Option<String> {
    for token in links.split(|ch: char| ch.is_whitespace() || ch == ';' || ch == ',') {
        let token = token.trim();
        if let Some(idx) = token.find("wikidata.org/wiki/Q") {
            let tail = &token[idx + "wikidata.org/wiki/".len()..];
            let qid: String = tail
                .chars()
                .take_while(|c| *c == 'Q' || c.is_ascii_digit())
                .collect();
            if qid.starts_with('Q') && qid.len() > 1 && qid[1..].chars().all(|c| c.is_ascii_digit()) {
                return Some(qid);
            }
        }
        if token.starts_with('Q') && token[1..].chars().all(|c| c.is_ascii_digit()) {
            return Some(token.to_string());
        }
    }
    None
}

pub fn parse_coordinates(raw: &str) -> Result<(f64, f64), RowReject> {
    let parts: Vec<&str> = raw.split(',').map(str::trim).collect();
    if parts.len() != 2 {
        return Err(RowReject::InvalidCoordinates);
    }
    let lat = parts[0].parse::<f64>().map_err(|_| RowReject::InvalidCoordinates)?;
    let lon = parts[1].parse::<f64>().map_err(|_| RowReject::InvalidCoordinates)?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err(RowReject::InvalidCoordinates);
    }
    Ok((lat, lon))
}

pub fn validate_reference(reference: &str) -> bool {
    let reference = reference.trim();
    reference.len() >= 3
        && reference
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn build_title(denomination: &str, address: &str, editorial: &str) -> String {
    for candidate in [denomination, address, editorial] {
        let trimmed = candidate.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    String::new()
}

#[derive(Debug, Clone)]
pub struct MerimeeColumns {
    indexes: HashMap<String, usize>,
}

impl MerimeeColumns {
    pub fn from_headers(headers: &csv::StringRecord) -> Result<Self, RowReject> {
        let mut indexes = HashMap::new();
        for (index, header) in headers.iter().enumerate() {
            indexes.insert(normalize_header_name(header), index);
        }
        if !indexes.contains_key("Reference") {
            return Err(RowReject::MalformedRow("missing Reference column".into()));
        }
        Ok(Self { indexes })
    }

    fn field<'a>(&self, record: &'a csv::StringRecord, names: &[&str]) -> Option<&'a str> {
        for name in names {
            if let Some(index) = self.indexes.get(*name) {
                return record.get(*index);
            }
        }
        None
    }
}

fn normalize_header_name(header: &str) -> String {
    header.trim().trim_start_matches('\u{feff}').to_string()
}

pub fn parse_csv_row(
    columns: &MerimeeColumns,
    record: &csv::StringRecord,
    row_number: u64,
) -> Result<ParsedMerimeeRow, RowReject> {
    let reference = columns.field(record, &["Reference"]).unwrap_or("").trim();
    if reference.is_empty() {
        return Err(RowReject::MissingReference);
    }
    if !validate_reference(reference) {
        return Err(RowReject::InvalidReference);
    }
    let denomination = columns
        .field(record, &["Denomination_de_l_edifice"])
        .unwrap_or("")
        .trim();
    let address = columns
        .field(record, &["Adresse_forme_index"])
        .unwrap_or("")
        .trim();
    let editorial = columns
        .field(record, &["Adresse_forme_editoriale"])
        .unwrap_or("")
        .trim();
    let title = build_title(denomination, address, editorial);
    if title.is_empty() {
        return Err(RowReject::MissingTitle);
    }
    let commune = non_empty(columns.field(record, &["Commune_forme_index"]));
    let department = non_empty(columns.field(record, &["Département", "Departement_format_numerique"]));
    let region = non_empty(columns.field(record, &["Région"]));
    let protection_kind = non_empty(columns.field(record, &["Nature_de_la_protection"]));
    let authors = non_empty(columns.field(record, &["Auteur_de_l_edifice"]));
    let historique = non_empty(columns.field(record, &["Historique"]));
    let description = non_empty(columns.field(record, &["Description_de_l_edifice"]));
    let personalities_raw = non_empty(columns.field(record, &["Personnes_liees_a_l_edifice"]));
    let external_links_raw = non_empty(columns.field(record, &["Liens_externes"]));
    let persons = personalities_raw
        .as_deref()
        .map(parse_personalities_field)
        .unwrap_or_default();
    let place_wikidata_qid = external_links_raw
        .as_deref()
        .and_then(extract_wikidata_qid);
    let coords_raw = columns
        .field(record, &["coordonnees_au_format_WGS84"])
        .unwrap_or("")
        .trim();
    let (lat, lon) = if coords_raw.is_empty() {
        (None, None)
    } else {
        match parse_coordinates(coords_raw) {
            Ok(pair) => (Some(pair.0), Some(pair.1)),
            Err(err) => return Err(err),
        }
    };
    let content_hash = notice_content_hash(
        reference,
        &title,
        commune.as_deref(),
        lat,
        lon,
        personalities_raw.as_deref(),
        protection_kind.as_deref(),
    );
    Ok(ParsedMerimeeRow {
        reference: reference.to_string(),
        title,
        commune,
        department,
        region,
        protection_kind,
        authors,
        historique,
        description,
        personalities_raw,
        persons,
        external_links_raw,
        place_wikidata_qid,
        lat,
        lon,
        content_hash,
        row_number,
    })
}

fn non_empty(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() { None } else { Some(value.to_string()) }
}

pub fn provenance_for_row(source_uri: &str, row_number: u64) -> serde_json::Value {
    json!({
        "source": "merimee_csv",
        "dataset": DATASET_URI,
        "source_uri": source_uri,
        "row_number": row_number,
    })
}

/// Stream-parse CSV from `reader`. Invokes `on_row` for each valid row.
pub fn stream_csv<R: Read, F, G>(
    reader: R,
    mut on_row: F,
    mut on_reject: G,
) -> Result<u64, csv::Error>
where
    F: FnMut(ParsedMerimeeRow),
    G: FnMut(u64, RowReject),
{
    let mut csv_reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .flexible(true)
        .from_reader(reader);
    let headers = csv_reader.headers()?.clone();
    let columns = match MerimeeColumns::from_headers(&headers) {
        Ok(columns) => columns,
        Err(reason) => {
            on_reject(1, reason);
            return Ok(0);
        }
    };
    let mut row_number = 1_u64;
    for record in csv_reader.records() {
        row_number += 1;
        match record {
            Ok(record) => match parse_csv_row(&columns, &record, row_number) {
                Ok(parsed) => on_row(parsed),
                Err(reason) => on_reject(row_number, reason),
            },
            Err(err) => on_reject(row_number, RowReject::MalformedRow(err.to_string())),
        }
    }
    Ok(row_number - 1)
}

pub fn person_tokens_for_query(person_label: &str) -> Vec<String> {
    normalize_person_name(person_label)
        .split_whitespace()
        .filter(|token| token.len() >= 3)
        .map(str::to_string)
        .collect()
}

pub fn notice_matches_person_label(person_label: &str, persons: &[MerimeePersonEntry]) -> bool {
    persons
        .iter()
        .any(|entry| person_named_in(person_label, &entry.raw) || person_named_in(person_label, &entry.name_normalized))
}

fn fold(value: &str) -> String {
    value
        .to_lowercase()
        .replace('é', "e")
        .replace('è', "e")
        .replace('ê', "e")
        .replace('ë', "e")
        .replace('à', "a")
        .replace('â', "a")
        .replace('ù', "u")
        .replace('û', "u")
        .replace('ô', "o")
        .replace('î', "i")
        .replace('ï', "i")
        .replace('ç', "c")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_person_field_and_coords() {
        let persons = parse_personalities_field("Hugo Victor (écrivain, habitant célèbre)");
        assert_eq!(persons.len(), 1);
        assert_eq!(persons[0].name_normalized, "hugo victor");
        assert!(persons[0].role_label.as_deref().unwrap().contains("habitant"));
        let (lat, lon) = parse_coordinates("48.8548221740133, 2.36611679564894").unwrap();
        assert!((lat - 48.8548).abs() < 0.001);
        assert!((lon - 2.3661).abs() < 0.001);
    }

    #[test]
    fn rejects_bad_reference_and_coords() {
        assert!(!validate_reference(""));
        assert!(!validate_reference("!!!"));
        assert!(validate_reference("PA00086272"));
        assert!(matches!(
            parse_coordinates("not-coords"),
            Err(RowReject::InvalidCoordinates)
        ));
    }

    #[test]
    fn extracts_wikidata_from_links() {
        let qid = extract_wikidata_qid("https://www.wikidata.org/wiki/Q123456 extra").unwrap();
        assert_eq!(qid, "Q123456");
    }

    #[test]
    fn streams_fixture_rows() {
        let fixture = include_str!("../../../fixtures/merimee/sample.csv");
        let mut ok = 0;
        let mut bad = 0;
        stream_csv(
            fixture.as_bytes(),
            |_| ok += 1,
            |_, _| bad += 1,
        )
        .unwrap();
        assert_eq!(ok, 3);
        assert_eq!(bad, 2);
    }
}
