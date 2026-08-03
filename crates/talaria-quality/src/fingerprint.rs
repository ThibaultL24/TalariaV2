// crates/talaria-quality/src/fingerprint.rs
use sha2::{Digest, Sha256};

use crate::model::{EventCandidate, Mention, TypedTime};

pub fn normalize_surface(s: &str) -> String {
    s.trim()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn sha_hex(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            hasher.update(b"|");
        }
        hasher.update(p.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn mention_key(m: &Mention) -> String {
    format!(
        "{}:{}:{}",
        normalize_surface(&m.surface),
        m.entity_id.map(|u| u.to_string()).unwrap_or_default(),
        m.role.map(|r| r.as_str().to_string()).unwrap_or_default()
    )
}

/// Candidate-level fingerprint: idempotent extraction within a clause.
/// Uses snapshot + offsets (not volatile fragment UUIDs) so retries stay stable.
pub fn candidate_fingerprint(
    extractor_version: &str,
    subject_surface: &str,
    event_type: &str,
    predicate: &str,
    time: &TypedTime,
    place_label: Option<&str>,
    snapshot_id: &str,
    clause_index: i32,
    start_offset: i32,
    end_offset: i32,
    participants: &[Mention],
) -> String {
    let mut parts_participants: Vec<String> = participants.iter().map(mention_key).collect();
    parts_participants.sort();
    let participants_joined = parts_participants.join(",");
    sha_hex(&[
        extractor_version,
        &normalize_surface(subject_surface),
        event_type,
        predicate,
        &time.canonical_key(),
        &normalize_surface(place_label.unwrap_or("")),
        snapshot_id,
        &clause_index.to_string(),
        &start_offset.to_string(),
        &end_offset.to_string(),
        &participants_joined,
    ])
}

/// Canonical event fingerprint: assemble idempotence (no title).
pub fn event_fingerprint(
    subject_entity_id: &str,
    event_type: &str,
    predicate: &str,
    time: &TypedTime,
    place_entity_id: Option<&str>,
    participant_entity_ids: &[String],
) -> String {
    let mut ids = participant_entity_ids.to_vec();
    ids.sort();
    sha_hex(&[
        "event",
        subject_entity_id,
        event_type,
        predicate,
        &time.canonical_key(),
        place_entity_id.unwrap_or(""),
        &ids.join(","),
    ])
}

pub fn fingerprint_from_candidate(c: &EventCandidate) -> String {
    candidate_fingerprint(
        &c.extractor_version,
        &c.subject_surface,
        &c.event_type,
        &c.predicate,
        &c.time,
        c.place_label.as_deref(),
        &c.snapshot_id.to_string(),
        c.clause_index,
        c.evidence_ptrs
            .first()
            .map(|e| e.start_offset)
            .unwrap_or(0),
        c.evidence_ptrs.first().map(|e| e.end_offset).unwrap_or(0),
        &c.participant_mentions,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TypedTime;

    #[test]
    fn stable_across_retry() {
        let t = TypedTime::Exact {
            year: 1821,
            month: None,
            day: None,
            surface: Some("1821".into()),
        };
        let a = candidate_fingerprint(
            "deterministic:clause_v1",
            "Napoleon",
            "death",
            "died_in",
            &t,
            Some("Saint Helena"),
            "snap-1",
            0,
            0,
            40,
            &[],
        );
        let b = candidate_fingerprint(
            "deterministic:clause_v1",
            "Napoleon",
            "death",
            "died_in",
            &t,
            Some("Saint Helena"),
            "snap-1",
            0,
            0,
            40,
            &[],
        );
        assert_eq!(a, b);
    }
}
