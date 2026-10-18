// crates/talaria-sources/src/snac/assertion.rs
//! Versioned identity of one SNAC relation assertion.
//!
//! `snac-assertion-key:v1` is stable. Changing it requires a migration that rewrites keys.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use super::ark::canonical_ark;

pub const ASSERTION_KEY_VERSION: &str = "snac-assertion-key:v1";

#[derive(Debug, Clone, PartialEq)]
pub struct AssertionParts {
    pub snac_relation_id: Option<String>,
    pub assertion_text: Option<String>,
    pub dates: serde_json::Value,
    pub object_ark: String,
    pub source_predicate: String,
    pub subject_ark: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappedPredicate {
    Accepted(&'static str),
    Unmapped,
}

pub fn source_assertion_key(relation: &AssertionParts) -> String {
    if let Some(id) = relation
        .snac_relation_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        return format!("snac:{id}");
    }
    format!("sha256:{}", assertion_content_hash(relation))
}

pub fn assertion_content_hash(relation: &AssertionParts) -> String {
    hex::encode(Sha256::digest(
        canonical_assertion_json(relation).as_bytes(),
    ))
}

pub fn canonical_assertion_json(relation: &AssertionParts) -> String {
    let mut fields = BTreeMap::new();
    fields.insert(
        "assertion_text".to_string(),
        match relation.assertion_text.as_deref().and_then(normalize_text) {
            Some(text) => serde_json::Value::String(text),
            None => serde_json::Value::Null,
        },
    );
    fields.insert("dates".to_string(), normalize_dates(&relation.dates));
    fields.insert("object_ark".to_string(), ark_json(&relation.object_ark));
    fields.insert(
        "source_predicate".to_string(),
        serde_json::Value::String(relation.source_predicate.trim().to_string()),
    );
    fields.insert("subject_ark".to_string(), ark_json(&relation.subject_ark));
    fields.insert(
        "v".to_string(),
        serde_json::Value::String(ASSERTION_KEY_VERSION.to_string()),
    );
    serde_json::to_string(&fields).expect("assertion json is a string map")
}

pub fn map_predicate(source_predicate: &str, object_type: &str) -> MappedPredicate {
    let predicate = source_predicate.trim();
    let object_type = object_type.trim();
    let accepted = match (predicate, object_type) {
        ("correspondedWith", "person" | "family") => Some("corresponded_with"),
        ("associatedWith", "person" | "corporateBody" | "family") => Some("associated_with"),
        ("acquaintanceOf", "person") => Some("acquaintance_of"),
        ("parentOf", "person") => Some("parent_of"),
        ("childOf", "person") => Some("child_of"),
        ("siblingOf", "person") => Some("sibling_of"),
        ("spouseOf", "person") => Some("spouse_of"),
        ("memberOf", "corporateBody") => Some("member_of"),
        ("hasMember", "person") => Some("has_member"),
        ("founderOf", "corporateBody") => Some("founder_of"),
        ("foundedBy", "person" | "corporateBody") => Some("founded_by"),
        _ => None,
    };
    match accepted {
        Some(name) => MappedPredicate::Accepted(name),
        None => MappedPredicate::Unmapped,
    }
}

fn ark_json(raw: &str) -> serde_json::Value {
    match canonical_ark(raw) {
        Some(ark) => serde_json::Value::String(ark),
        None => serde_json::Value::Null,
    }
}

fn normalize_text(raw: &str) -> Option<String> {
    let mut normalized = String::new();
    let mut pending_space = false;
    for character in raw.trim().chars() {
        if character.is_whitespace() {
            pending_space = !normalized.is_empty();
            continue;
        }
        if pending_space {
            normalized.push('\u{0020}');
            pending_space = false;
        }
        normalized.push(character);
    }
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn normalize_dates(value: &serde_json::Value) -> serde_json::Value {
    let object = value.as_object();
    let kind = object
        .and_then(|fields| fields.get("kind"))
        .and_then(|kind| kind.as_str());
    let surface = object
        .and_then(|fields| fields.get("surface"))
        .and_then(|surface| surface.as_str())
        .and_then(normalize_text);
    if !matches!(kind, Some("exact" | "range" | "approx" | "unknown")) {
        return unknown_dates(surface);
    }
    let kind = kind.expect("kind was just matched");
    let mut fields = BTreeMap::new();
    fields.insert(
        "kind".to_string(),
        serde_json::Value::String(kind.to_string()),
    );
    if let Some(surface) = surface {
        fields.insert("surface".to_string(), serde_json::Value::String(surface));
    }
    if kind != "unknown" {
        if let Some(precision) = object
            .and_then(|fields| fields.get("precision"))
            .and_then(|precision| precision.as_str())
            .filter(|precision| matches!(*precision, "day" | "month" | "year"))
        {
            fields.insert(
                "precision".to_string(),
                serde_json::Value::String(precision.to_string()),
            );
        }
        for key in ["start", "end"] {
            if let Some(text) = object
                .and_then(|fields| fields.get(key))
                .and_then(|text| text.as_str())
                .and_then(normalize_text)
            {
                fields.insert(key.to_string(), serde_json::Value::String(text));
            }
        }
    }
    serde_json::to_value(fields).expect("date fields are a string map")
}

fn unknown_dates(surface: Option<String>) -> serde_json::Value {
    let mut fields = BTreeMap::new();
    fields.insert(
        "kind".to_string(),
        serde_json::Value::String("unknown".to_string()),
    );
    if let Some(surface) = surface {
        fields.insert("surface".to_string(), serde_json::Value::String(surface));
    }
    serde_json::to_value(fields).expect("unknown dates are a string map")
}

#[cfg(test)]
mod tests {
    use super::{
        assertion_content_hash, canonical_assertion_json, source_assertion_key, AssertionParts,
    };

    fn sample() -> AssertionParts {
        AssertionParts {
            snac_relation_id: None,
            assertion_text: None,
            dates: serde_json::json!({"kind": "unknown"}),
            object_ark: "ark:/99166/w6object".into(),
            source_predicate: "correspondedWith".into(),
            subject_ark: "ark:/99166/w6subject".into(),
        }
    }

    #[test]
    fn native_id_ignores_text_for_the_key_but_not_the_hash() {
        let mut first = sample();
        first.snac_relation_id = Some("42".into());
        let mut second = first.clone();
        second.assertion_text = Some("other".into());
        assert_eq!(source_assertion_key(&first), "snac:42");
        assert_eq!(source_assertion_key(&second), "snac:42");
        assert_ne!(
            assertion_content_hash(&first),
            assertion_content_hash(&second)
        );
    }

    #[test]
    fn missing_id_changes_key_when_text_changes() {
        let mut first = sample();
        first.snac_relation_id = None;
        let mut second = first.clone();
        second.assertion_text = Some("other".into());
        assert_ne!(source_assertion_key(&first), source_assertion_key(&second));
        assert!(source_assertion_key(&first).starts_with("sha256:"));
    }

    #[test]
    fn whitespace_and_ark_prefix_do_not_change_the_key() {
        let mut first = sample();
        let mut second = first.clone();
        first.assertion_text = Some("met  at\nParis".into());
        second.assertion_text = Some("met at Paris".into());
        first.subject_ark = "https://n2t.net/ark:/99166/w6subject".into();
        second.subject_ark = "ark:/99166/w6subject".into();
        assert_eq!(source_assertion_key(&first), source_assertion_key(&second));
    }

    #[test]
    fn canonical_json_matches_the_locked_shape() {
        let json = canonical_assertion_json(&sample());
        assert_eq!(
            json,
            r#"{"assertion_text":null,"dates":{"kind":"unknown"},"object_ark":"ark:/99166/w6object","source_predicate":"correspondedWith","subject_ark":"ark:/99166/w6subject","v":"snac-assertion-key:v1"}"#
        );
    }
}

#[cfg(test)]
mod predicate {
    use super::{map_predicate, MappedPredicate};

    #[test]
    fn corresponded_with_requires_a_person_or_family() {
        assert_eq!(
            map_predicate("correspondedWith", "person"),
            MappedPredicate::Accepted("corresponded_with")
        );
        assert_eq!(
            map_predicate("correspondedWith", "corporateBody"),
            MappedPredicate::Unmapped
        );
    }

    #[test]
    fn associated_with_is_not_correspondence() {
        assert_eq!(
            map_predicate("associatedWith", "person"),
            MappedPredicate::Accepted("associated_with")
        );
    }

    #[test]
    fn member_of_requires_a_corporate_body() {
        assert_eq!(
            map_predicate("memberOf", "corporateBody"),
            MappedPredicate::Accepted("member_of")
        );
    }
}
