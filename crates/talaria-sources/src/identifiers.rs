// crates/talaria-sources/src/identifiers.rs
//! Normalization for bibliographic identifier schemes.

use crate::kinds::IdentifierScheme;

pub fn normalize_identifier(scheme: IdentifierScheme, raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    match scheme {
        IdentifierScheme::Nnt => normalize_nnt(trimmed),
        IdentifierScheme::Ppn => normalize_ppn(trimmed),
        IdentifierScheme::Doi => Some(normalize_doi(trimmed)),
        IdentifierScheme::Isbn10 | IdentifierScheme::Isbn13 => normalize_isbn(trimmed, scheme),
        IdentifierScheme::Ark => Some(trimmed.to_ascii_lowercase()),
        IdentifierScheme::HalId => Some(trimmed.to_ascii_lowercase()),
        IdentifierScheme::NumSujet => Some(normalize_num_sujet(trimmed)),
        IdentifierScheme::Oclc | IdentifierScheme::Olid | IdentifierScheme::Other => {
            Some(trimmed.to_ascii_lowercase())
        }
    }
}

fn normalize_nnt(raw: &str) -> Option<String> {
    let compact: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase();
    if compact.is_empty() {
        None
    } else {
        Some(compact)
    }
}

fn normalize_ppn(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        Some(digits)
    }
}

fn normalize_doi(raw: &str) -> String {
    let lower = raw.trim().to_ascii_lowercase();
    lower
        .strip_prefix("https://doi.org/")
        .or_else(|| lower.strip_prefix("http://doi.org/"))
        .or_else(|| lower.strip_prefix("doi:"))
        .unwrap_or(&lower)
        .trim()
        .to_string()
}

fn normalize_isbn(raw: &str, scheme: IdentifierScheme) -> Option<String> {
    let compact: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase();
    match scheme {
        IdentifierScheme::Isbn10 if compact.len() == 10 => Some(compact),
        IdentifierScheme::Isbn13 if compact.len() == 13 => Some(compact),
        _ if compact.len() == 10 || compact.len() == 13 => Some(compact),
        _ => None,
    }
}

fn normalize_num_sujet(raw: &str) -> String {
    let t = raw.trim().to_ascii_lowercase();
    if t.starts_with('s') {
        t
    } else {
        format!("s{t}")
    }
}

pub fn normalize_person_name(nom: &str, prenom: Option<&str>) -> String {
    let mut parts = Vec::new();
    if let Some(p) = prenom.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(p.to_ascii_lowercase());
    }
    let n = nom.trim();
    if !n.is_empty() {
        parts.push(n.to_ascii_lowercase());
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nnt_strips_noise_and_uppercases() {
        assert_eq!(
            normalize_identifier(IdentifierScheme::Nnt, "2020abcd1234").as_deref(),
            Some("2020ABCD1234")
        );
    }

    #[test]
    fn doi_strips_resolver_prefix() {
        assert_eq!(
            normalize_identifier(IdentifierScheme::Doi, "https://doi.org/10.1234/AbC").as_deref(),
            Some("10.1234/abc")
        );
    }

    #[test]
    fn ppn_digits_only() {
        assert_eq!(
            normalize_identifier(IdentifierScheme::Ppn, "PPN 123-456").as_deref(),
            Some("123456")
        );
    }
}
