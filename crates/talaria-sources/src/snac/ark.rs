// crates/talaria-sources/src/snac/ark.rs
//! Canonical ARK and sameAs sets. Two values of one scheme are a conflict, not a choice.

use std::collections::{BTreeMap, BTreeSet};

const ARK_PREFIXES: &[&str] = &[
    "https://www.snaccooperative.org/",
    "http://www.snaccooperative.org/",
    "https://snaccooperative.org/",
    "http://snaccooperative.org/",
    "https://n2t.net/",
    "http://n2t.net/",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SameAs {
    pub values: BTreeMap<String, BTreeSet<String>>,
    pub raw: BTreeSet<String>,
    pub internal_conflict: bool,
}

pub fn canonical_ark(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    let rest = ARK_PREFIXES
        .iter()
        .find_map(|prefix| lower.strip_prefix(prefix))
        .unwrap_or(lower.as_str());
    if rest.starts_with("ark:/") {
        Some(rest.to_string())
    } else {
        None
    }
}

pub fn normalize_qid(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let token = trimmed
        .rsplit(['/', '#'])
        .next()
        .unwrap_or(trimmed)
        .trim();
    let mut chars = token.chars();
    let q = chars.next()?;
    if !q.eq_ignore_ascii_case(&'Q') {
        return None;
    }
    let digits: String = chars.collect();
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("Q{digits}"))
}

pub fn normalize_viaf(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        Some(digits)
    }
}

pub fn normalize_isni(raw: &str) -> Option<String> {
    let compact: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase();
    if compact.len() == 16 {
        Some(compact)
    } else {
        None
    }
}

pub fn parse_same_as(uris: &[&str]) -> SameAs {
    let mut values: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut raw = BTreeSet::new();
    for uri in uris {
        let trimmed = uri.trim();
        if trimmed.is_empty() {
            continue;
        }
        raw.insert(trimmed.to_string());
        if let Some((scheme, value)) = classify_uri(trimmed) {
            values.entry(scheme).or_default().insert(value);
        }
    }
    let internal_conflict = values.values().any(|set| set.len() > 1);
    SameAs {
        values,
        raw,
        internal_conflict,
    }
}

fn classify_uri(uri: &str) -> Option<(String, String)> {
    let lower = uri.to_ascii_lowercase();
    if lower.contains("wikidata.org") {
        return normalize_qid(uri).map(|qid| ("wikidata".to_string(), qid));
    }
    if lower.contains("viaf.org") {
        return normalize_viaf(uri).map(|viaf| ("viaf".to_string(), viaf));
    }
    if lower.contains("isni.org") {
        let token = uri.rsplit(['/', '#']).next().unwrap_or(uri);
        return normalize_isni(token).map(|isni| ("isni".to_string(), isni));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{canonical_ark, parse_same_as};

    #[test]
    fn ark_prefixes_collapse() {
        let a = canonical_ark("https://n2t.net/ark:/99166/w6abc").unwrap();
        let b = canonical_ark("ark:/99166/W6ABC").unwrap();
        assert_eq!(a, "ark:/99166/w6abc");
        assert_eq!(a, b);
        assert!(canonical_ark("https://example.org/not-an-ark").is_none());
    }

    #[test]
    fn two_wikidata_values_conflict() {
        let same = parse_same_as(&[
            "http://www.wikidata.org/entity/Q517",
            "http://www.wikidata.org/entity/Q42",
        ]);
        assert!(same.internal_conflict);
        assert_eq!(same.values.get("wikidata").unwrap().len(), 2);
    }
}
