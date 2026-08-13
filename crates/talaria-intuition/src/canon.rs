// crates/talaria-intuition/src/canon.rs
//! Deterministic slugs + debate bundles (POC Canonicalizer port).

use deunicode::deunicode;
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const SCHEMA_VERSION: &str = "talaria.intuition_canon.v1";

pub const VOCAB_ATOM_KINDS: &[&str] = &[
    "question",
    "proposition",
    "source",
    "person",
    "date",
    "debate",
    "theme",
    "event",
    "place",
    "predicate",
];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CanonError {
    #[error("unknown atom kind: {0}")]
    InvalidKind(String),
    #[error("invalid slug (missing ':'): {0}")]
    InvalidSlug(String),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AtomRecord {
    pub id: String,
    pub kind: String,
    pub slug_fragment: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub on_chain_data: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TripleRecord {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub role: String,
    pub fact_kind: String,
    pub fingerprint: String,
    pub dedupe_key: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DebateBundle {
    pub version: String,
    pub debate_id: String,
    pub atoms: Vec<AtomRecord>,
    pub triples: Vec<TripleRecord>,
    pub vote_target: VoteTarget,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub situated_context: Option<SituatedContext>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VoteTarget {
    pub triple_slug: String,
    pub triple_fingerprint: String,
    pub proposition_atom_id: String,
    pub question_atom_id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SituatedContext {
    pub canonical_event_id: String,
    pub triple_roles: Vec<String>,
}

pub fn normalize_slug_fragment(raw: &str) -> String {
    let ascii = deunicode(raw).to_lowercase();
    let mut out = String::new();
    let mut prev_hyphen = false;
    for ch in ascii.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_hyphen = false;
        } else if !prev_hyphen {
            out.push('-');
            prev_hyphen = true;
        }
    }
    let s = out.trim_matches('-').to_string();
    if s.is_empty() {
        "x".into()
    } else {
        s
    }
}

pub fn validate_kind(kind: &str) -> Result<&str, CanonError> {
    if VOCAB_ATOM_KINDS.contains(&kind) {
        Ok(kind)
    } else {
        Err(CanonError::InvalidKind(kind.to_string()))
    }
}

pub fn full_slug(kind: &str, fragment: &str) -> Result<String, CanonError> {
    let k = validate_kind(kind)?;
    Ok(format!("{k}:{}", normalize_slug_fragment(fragment)))
}

pub fn parse_full_slug(slug: &str) -> Result<(String, String), CanonError> {
    let Some((kind, fragment)) = slug.split_once(':') else {
        return Err(CanonError::InvalidSlug(slug.to_string()));
    };
    validate_kind(kind)?;
    Ok((kind.to_string(), fragment.to_string()))
}

pub fn fingerprint_hex(payload: &serde_json::Value) -> String {
    let sorted = sort_value(payload);
    let bytes = serde_json::to_vec(&sorted).unwrap_or_else(|_| b"{}".to_vec());
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    hex::encode(hasher.finalize())
}

fn sort_value(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();
            let mut out = serde_json::Map::new();
            for k in keys {
                if let Some(val) = map.get(&k) {
                    out.insert(k, sort_value(val));
                }
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(sort_value).collect())
        }
        other => other.clone(),
    }
}

pub fn atom_record(kind: &str, fragment: &str, label: Option<&str>) -> Result<AtomRecord, CanonError> {
    let k = validate_kind(kind)?;
    let slug_fragment = normalize_slug_fragment(fragment);
    let slug = format!("{k}:{slug_fragment}");
    let fingerprint = fingerprint_hex(&serde_json::json!({
        "v": 1,
        "kind": k,
        "fragment": slug_fragment,
    }));
    Ok(AtomRecord {
        id: slug.clone(),
        kind: k.to_string(),
        slug_fragment,
        label: label.map(str::to_string),
        on_chain_data: slug,
        fingerprint,
    })
}

pub fn predicate_atom(key: &str) -> Result<AtomRecord, CanonError> {
    let frag = match key {
        "has_proposition" => "has-proposition",
        "supported_by" => "supported-by",
        "authored_by" => "authored-by",
        "dated" => "dated",
        "about" => "about",
        "at" => "at",
        other => other,
    };
    atom_record("predicate", frag, Some(&key.replace('_', " ")))
}

pub fn triple_record(
    subject_slug: &str,
    predicate_slug: &str,
    object_slug: &str,
    role: &str,
    fact_kind: &str,
) -> Result<TripleRecord, CanonError> {
    parse_full_slug(subject_slug)?;
    parse_full_slug(predicate_slug)?;
    parse_full_slug(object_slug)?;
    let fingerprint = fingerprint_hex(&serde_json::json!({
        "v": 1,
        "subject": subject_slug,
        "predicate": predicate_slug,
        "object": object_slug,
        "role": role,
    }));
    Ok(TripleRecord {
        subject: subject_slug.to_string(),
        predicate: predicate_slug.to_string(),
        object: object_slug.to_string(),
        role: role.to_string(),
        fact_kind: fact_kind.to_string(),
        fingerprint,
        dedupe_key: format!("{subject_slug}|{predicate_slug}|{object_slug}"),
    })
}

pub fn situated_context_triples(
    proposition_slug: &str,
    event_id: &str,
    event_title: Option<&str>,
    place_name: Option<&str>,
) -> Result<(Vec<AtomRecord>, Vec<TripleRecord>), CanonError> {
    let event_atom = atom_record(
        "event",
        &format!("canonical-event-{event_id}"),
        Some(event_title.unwrap_or("Canonical event")),
    )?;
    let pred_about = predicate_atom("about")?;
    let mut atoms = vec![event_atom.clone(), pred_about.clone()];
    let mut triples = vec![triple_record(
        proposition_slug,
        &pred_about.id,
        &event_atom.id,
        "proposition_about_event",
        "semantic",
    )?];
    if let Some(place) = place_name.filter(|p| !p.trim().is_empty()) {
        let place_atom = atom_record("place", place, Some(place))?;
        let pred_at = predicate_atom("at")?;
        if !atoms.iter().any(|a| a.id == place_atom.id) {
            atoms.push(place_atom.clone());
        }
        if !atoms.iter().any(|a| a.id == pred_at.id) {
            atoms.push(pred_at.clone());
        }
        triples.push(triple_record(
            &event_atom.id,
            &pred_at.id,
            &place_atom.id,
            "event_at_place",
            "semantic",
        )?);
    }
    Ok((atoms, triples))
}

pub fn build_debate_bundle(
    debate_id: &str,
    question_fragment: &str,
    question_title: &str,
    proposition_fragment: &str,
    proposition_title: &str,
) -> Result<DebateBundle, CanonError> {
    let q = atom_record("question", question_fragment, Some(question_title))?;
    let p = atom_record("proposition", proposition_fragment, Some(proposition_title))?;
    let pred = predicate_atom("has_proposition")?;
    let main = triple_record(
        &q.id,
        &pred.id,
        &p.id,
        "question_has_proposition",
        "semantic",
    )?;
    Ok(DebateBundle {
        version: SCHEMA_VERSION.into(),
        debate_id: debate_id.to_string(),
        atoms: vec![q.clone(), p.clone(), pred],
        triples: vec![main.clone()],
        vote_target: VoteTarget {
            triple_slug: main.dedupe_key.clone(),
            triple_fingerprint: main.fingerprint,
            proposition_atom_id: p.id,
            question_atom_id: q.id,
        },
        situated_context: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_slug_fragment_transliterates_and_hyphenates() {
        assert_eq!(
            normalize_slug_fragment("La Révolution française!!!"),
            "la-revolution-francaise"
        );
    }

    #[test]
    fn full_slug_rejects_unknown_kind() {
        assert!(matches!(
            full_slug("unknown", "x"),
            Err(CanonError::InvalidKind(_))
        ));
    }

    #[test]
    fn full_slug_is_stable_for_equivalent_inputs() {
        let a = full_slug("question", "  Foo  Bar  ").unwrap();
        let b = full_slug("question", "foo-bar").unwrap();
        assert_eq!(a, "question:foo-bar");
        assert_eq!(a, b);
    }

    #[test]
    fn fingerprint_hex_is_deterministic() {
        let p1 = serde_json::json!({"v": 1, "a": 1, "b": {"z": 1, "y": 2}});
        let p2 = serde_json::json!({"b": {"y": 2, "z": 1}, "a": 1, "v": 1});
        assert_eq!(fingerprint_hex(&p1), fingerprint_hex(&p2));
    }

    #[test]
    fn build_debate_bundle_produces_stable_vote_target() {
        let b1 = build_debate_bundle(
            "talaria:debate:revolution",
            "revolution-francaise-etat-moderne",
            "Q?",
            "oui-centralisation",
            "Oui",
        )
        .unwrap();
        let b2 = build_debate_bundle(
            "talaria:debate:revolution",
            "revolution-francaise-etat-moderne",
            "Autre titre",
            "oui-centralisation",
            "Autre",
        )
        .unwrap();
        assert_eq!(
            b1.vote_target.triple_fingerprint,
            b2.vote_target.triple_fingerprint
        );
        assert_eq!(b1.vote_target.triple_slug, b2.vote_target.triple_slug);
    }

    #[test]
    fn situated_context_triples_links_proposition_to_event_and_place() {
        let (atoms, triples) = situated_context_triples(
            "proposition:oui-centralisation",
            "42",
            Some("Austerlitz"),
            Some("Moravia"),
        )
        .unwrap();
        let mut roles: Vec<_> = triples.iter().map(|t| t.role.as_str()).collect();
        roles.sort();
        assert_eq!(roles, vec!["event_at_place", "proposition_about_event"]);
        assert!(atoms.iter().any(|a| a.id == "event:canonical-event-42"));
    }
}
