// crates/talaria-intuition/src/fact.rs
//! DebateFact v2 — Rust shapes opinions; the sidecar owns the graph.

use serde::Serialize;

use crate::canon::fingerprint_hex;
use crate::plan::{ConflictGroup, SoftClaimInput};
use crate::normalize_slug_fragment;

pub const SCHEMA_VERSION_V2: &str = "talaria.intuition_canon.v2";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DebateText {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AboutEvent {
    pub canonical_event_id: String,
    pub title: String,
    pub event_type: String,
    pub time_surface: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DebateFact {
    pub version: String,
    pub debate_id: String,
    pub kind: String,
    pub question: DebateText,
    pub proposition: DebateText,
    pub about_event: Option<AboutEvent>,
}

pub fn fact_fingerprint(fact: &DebateFact) -> String {
    fingerprint_hex(&serde_json::json!({
        "version": fact.version,
        "kind": fact.kind,
        "question": fact.question.text,
        "proposition": fact.proposition.text,
        "canonical_event_id": fact.about_event.as_ref().map(|e| e.canonical_event_id.as_str()),
    }))
}

pub fn category_term(fact: &DebateFact) -> String {
    if let Some(ev) = &fact.about_event {
        let t = ev.event_type.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    let k = fact.kind.trim();
    if k.is_empty() || k == "place_conflict" {
        "uncategorized".into()
    } else {
        k.to_string()
    }
}

pub fn start_date_field(time_surface: &str) -> Option<&str> {
    let s = time_surface.trim();
    let mut parts = s.split('-');
    let y = parts.next()?;
    let m = parts.next()?;
    let d = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    if y.len() == 4
        && m.len() == 2
        && d.len() == 2
        && y.chars().all(|c| c.is_ascii_digit())
        && m.chars().all(|c| c.is_ascii_digit())
        && d.chars().all(|c| c.is_ascii_digit())
    {
        Some(s)
    } else {
        None
    }
}

pub fn event_atom_name(canonical_event_id: &str) -> String {
    format!("canonical-event:{canonical_event_id}")
}

pub fn event_same_as(canonical_event_id: &str) -> String {
    format!("talaria://canonical-event/{canonical_event_id}")
}

pub fn fact_from_place_conflict(group: &ConflictGroup, place: &str) -> DebateFact {
    let q_frag = format!(
        "where-{}-{}-{}",
        group.subject_label, group.event_type, group.time_key
    );
    let p_frag = format!("at-{place}");
    let debate_id = format!(
        "talaria:debate:{}:{}",
        normalize_slug_fragment(&q_frag),
        normalize_slug_fragment(&p_frag)
    );
    let question = format!(
        "Where was {} during {} ({})?",
        group.subject_label, group.event_type, group.time_key
    );
    DebateFact {
        version: SCHEMA_VERSION_V2.into(),
        debate_id,
        kind: "place_conflict".into(),
        question: DebateText { text: question },
        proposition: DebateText {
            text: place.to_string(),
        },
        about_event: group.event_id.as_ref().map(|id| AboutEvent {
            canonical_event_id: id.clone(),
            title: group
                .event_title
                .clone()
                .unwrap_or_else(|| format!("{} {}", group.subject_label, group.event_type)),
            event_type: group.event_type.clone(),
            time_surface: group.time_key.clone(),
        }),
    }
}

pub fn fact_from_soft_claim(claim: &SoftClaimInput) -> DebateFact {
    let q_frag = format!("about-{}", claim.subject_label);
    let p_frag = format!("claim-{}", claim.claim_id);
    let debate_id = format!(
        "talaria:debate:{}:{}",
        normalize_slug_fragment(&q_frag),
        normalize_slug_fragment(&p_frag)
    );
    DebateFact {
        version: SCHEMA_VERSION_V2.into(),
        debate_id,
        kind: claim.claim_kind.clone(),
        question: DebateText {
            text: format!("What is claimed about {}?", claim.subject_label),
        },
        proposition: DebateText {
            text: claim.text.clone(),
        },
        about_event: claim.event_id.as_ref().map(|id| AboutEvent {
            canonical_event_id: id.clone(),
            title: claim.event_title.clone().unwrap_or_default(),
            event_type: claim.event_type.clone().unwrap_or_default(),
            time_surface: claim.time_surface.clone().unwrap_or_default(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn austerlitz() -> DebateFact {
        DebateFact {
            version: SCHEMA_VERSION_V2.into(),
            debate_id: "talaria:debate:napoleon-battle-1805:at-austerlitz".into(),
            kind: "place_conflict".into(),
            question: DebateText {
                text: "Where was Napoleon during battle (1805)?".into(),
            },
            proposition: DebateText {
                text: "Austerlitz".into(),
            },
            about_event: Some(AboutEvent {
                canonical_event_id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into(),
                title: "Battle of Austerlitz".into(),
                event_type: "battle".into(),
                time_surface: "1805".into(),
            }),
        }
    }

    #[test]
    fn fingerprint_ignores_display_title() {
        let mut a = austerlitz();
        let mut b = austerlitz();
        b.about_event.as_mut().unwrap().title = "Another label".into();
        assert_eq!(fact_fingerprint(&a), fact_fingerprint(&b));
        a.proposition.text = "Slavkov".into();
        assert_ne!(fact_fingerprint(&a), fact_fingerprint(&b));
    }

    #[test]
    fn category_prefers_event_type_then_kind() {
        assert_eq!(category_term(&austerlitz()), "battle");
        let mut theory = austerlitz();
        theory.kind = "theory".into();
        theory.about_event = None;
        assert_eq!(category_term(&theory), "theory");
        theory.kind = "place_conflict".into();
        assert_eq!(category_term(&theory), "uncategorized");
    }

    #[test]
    fn year_only_surface_is_not_coerced_to_january_first() {
        assert_eq!(start_date_field("1805"), None);
        assert_eq!(start_date_field("1805-12"), None);
        assert_eq!(start_date_field("1805-12-02"), Some("1805-12-02"));
        assert_eq!(
            event_atom_name("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"),
            "canonical-event:aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"
        );
    }

    #[test]
    fn place_conflict_fact_does_not_embed_chosen_place_on_event() {
        let group = ConflictGroup {
            subject_label: "Napoleon".into(),
            occurrence_stem: "stem".into(),
            event_type: "battle".into(),
            time_key: "1805".into(),
            places: vec!["Austerlitz".into(), "Vienna".into()],
            event_id: Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into()),
            event_title: Some("Battle of Austerlitz".into()),
        };
        let fact = fact_from_place_conflict(&group, "Austerlitz");
        assert_eq!(fact.kind, "place_conflict");
        assert_eq!(fact.proposition.text, "Austerlitz");
        let ev = fact.about_event.unwrap();
        assert_eq!(ev.event_type, "battle");
        assert_eq!(ev.time_surface, "1805");
        assert_eq!(ev.canonical_event_id, "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
    }
}
