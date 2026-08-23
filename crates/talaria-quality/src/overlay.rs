// crates/talaria-quality/src/overlay.rs
//! LLM overlay verdicts: drop / un-place / relabel. Never invent events or coords.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayItem {
    pub i: usize,
    pub event_type: String,
    pub year: Option<i32>,
    pub place: Option<String>,
    pub clause: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayVerdict {
    pub i: usize,
    #[serde(default = "bool_true")]
    pub keep_timeline: bool,
    #[serde(default = "bool_true")]
    pub keep_map: bool,
    #[serde(default = "bool_true")]
    pub place_ok: bool,
    #[serde(default)]
    pub event_type: Option<String>,
    #[serde(default)]
    pub reason: String,
}

fn bool_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayEffect {
    pub drop: bool,
    pub strip_place: bool,
    pub event_type: Option<String>,
}

pub fn overlay_effect(v: &OverlayVerdict) -> OverlayEffect {
    OverlayEffect {
        drop: !v.keep_timeline,
        strip_place: !v.keep_map || !v.place_ok,
        event_type: v
            .event_type
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
    }
}

/// Parse a JSON array of verdicts, including fenced / noisy model output.
pub fn parse_overlay_verdicts(raw: &str) -> Result<Vec<OverlayVerdict>, String> {
    let trimmed = raw.trim();
    let json = extract_json_array(trimmed).ok_or_else(|| "no JSON array in overlay output".to_string())?;
    serde_json::from_str(json).map_err(|e| e.to_string())
}

fn extract_json_array(s: &str) -> Option<&str> {
    let start = s.find('[')?;
    let end = s.rfind(']')?;
    if end > start {
        Some(&s[start..=end])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_strips_fences_and_applies_drop_unplace() {
        let raw = r#"```json
[{"i":0,"keep_timeline":false,"keep_map":true,"place_ok":true,"event_type":null,"reason":"third party"},
 {"i":1,"keep_timeline":true,"keep_map":false,"place_ok":false,"event_type":"meeting","reason":"not a place"}]
```"#;
        let vs = parse_overlay_verdicts(raw).expect("parse");
        assert_eq!(vs.len(), 2);
        assert_eq!(
            overlay_effect(&vs[0]),
            OverlayEffect {
                drop: true,
                strip_place: false,
                event_type: None
            }
        );
        assert_eq!(
            overlay_effect(&vs[1]),
            OverlayEffect {
                drop: false,
                strip_place: true,
                event_type: Some("meeting".into())
            }
        );
    }

    #[test]
    fn missing_flags_fail_open() {
        let vs = parse_overlay_verdicts(r#"[{"i":0}]"#).unwrap();
        let e = overlay_effect(&vs[0]);
        assert!(!e.drop);
        assert!(!e.strip_place);
    }
}
