//! Versioned contracts for evidence-bound AI processing. No canonical writes.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const EXTRACTION_VERSION: &str = "person-extraction:v3:1";

pub fn cache_key(provider: &str, model: &str, subject: &str, title: &str, text: &str) -> String {
    // JSON encoding preserves boundaries and avoids concatenation collisions.
    hex::encode(Sha256::digest(
        serde_json::to_vec(&(EXTRACTION_VERSION, provider, model, subject, title, text))
            .expect("strings serialize"),
    ))
}

pub fn extraction_schema() -> Value {
    json!({
        "type": "object", "additionalProperties": false,
        "required": ["items"],
        "properties": {"items": {"type": "array", "items": {
            "type": "object", "additionalProperties": false,
            "required": ["lane", "event_type", "role", "year", "place_surface", "summary", "quoted_text", "confidence"],
            "properties": {
                "lane": {"type": "string", "enum": ["fact", "debate"]},
                "event_type": {"type": "string", "enum": ["birth", "death", "residence", "travel", "battle", "treaty", "diplomatic", "office", "education", "work", "anecdote", "commemoration", "other"]},
                "role": {"type": "string", "enum": ["direct", "indirect"]},
                "year": {"type": ["integer", "null"]},
                "place_surface": {"type": ["string", "null"]},
                "summary": {"type": "string"},
                "quoted_text": {"type": "string"},
                "confidence": {"type": "number", "minimum": 0, "maximum": 1}
            }
        }}}
    })
}

/// Verify semantics locally even when a provider advertises constrained decoding.
pub fn validate_extraction(value: &Value, text: &str) -> anyhow::Result<()> {
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("missing extraction items"))?;
    anyhow::ensure!(items.len() <= 128, "too many extraction items");
    for item in items {
        let quote = item
            .get("quoted_text")
            .and_then(Value::as_str)
            .unwrap_or("");
        anyhow::ensure!(
            !quote.trim().is_empty() && text.contains(quote),
            "ungrounded extraction quote"
        );
        let score = item
            .get("confidence")
            .and_then(Value::as_f64)
            .unwrap_or(-1.0);
        anyhow::ensure!((0.0..=1.0).contains(&score), "invalid confidence");
        for field in ["lane", "event_type", "role"] {
            let schema = extraction_schema();
            let allowed = &schema["properties"]["items"]["items"]["properties"][field]["enum"];
            anyhow::ensure!(
                allowed.as_array().unwrap().contains(&item[field]),
                "invalid {field}"
            );
        }
        anyhow::ensure!(item["summary"].is_string(), "invalid summary");
        anyhow::ensure!(
            item["year"].is_null() || item["year"].as_i64().is_some(),
            "invalid year"
        );
        anyhow::ensure!(
            item["place_surface"].is_null() || item["place_surface"].is_string(),
            "invalid place"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_preserves_boundaries_and_model() {
        assert_ne!(
            cache_key("openai", "m", "ab", "c", "d"),
            cache_key("openai", "m", "a", "bc", "d")
        );
        assert_ne!(
            cache_key("openai", "m", "a", "b", "c"),
            cache_key("openai", "n", "a", "b", "c")
        );
    }
    #[test]
    fn invented_evidence_is_rejected() {
        let item = json!({"items":[{"lane":"fact", "event_type":"travel", "role":"direct", "year":1855, "place_surface":"Guernsey", "summary":"Arrival", "quoted_text":"Hugo arrived in Guernsey.", "confidence":0.9}]});
        assert!(validate_extraction(&item, "Hugo arrived in Guernsey.").is_ok());
        assert!(validate_extraction(&item, "Hugo stayed in Paris.").is_err());
    }
}
