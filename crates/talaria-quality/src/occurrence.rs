// crates/talaria-quality/src/occurrence.rs
//! Occurrence keys — historical identity shared by ingest and Lot E.
//! Distinct from candidate_fingerprint (snapshot-local extraction idempotence).

use sha2::{Digest, Sha256};

use crate::fingerprint::normalize_surface;
use crate::model::TypedTime;

/// Structured key distinguishing historically distinct occurrences.
///
/// Do **not** pass extractor id as `primary_document` — that would split one
/// historical fact across extractors. `primary_document` is reserved for a
/// bibliographic edition when the occurrence is about a specific work.
pub fn occurrence_key(
    subject: &str,
    event_type: &str,
    action_role: &str,
    time: &TypedTime,
    place: Option<&str>,
    primary_object: Option<&str>,
    primary_document: Option<&str>,
    sequence_marker: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        normalize_surface(subject),
        event_type.to_string(),
        action_role.to_string(),
        time.canonical_key(),
        normalize_surface(place.unwrap_or("")),
        normalize_surface(primary_object.unwrap_or("")),
        normalize_surface(primary_document.unwrap_or("")),
        sequence_marker.unwrap_or("").to_string(),
    ] {
        hasher.update(part.as_bytes());
        hasher.update(b"|");
    }
    hex::encode(hasher.finalize())
}

/// Life-event / density occurrence: no document edition, no extractor id.
pub fn occurrence_key_for_event(
    subject: &str,
    event_type: &str,
    action_role: &str,
    time: &TypedTime,
    place: Option<&str>,
    primary_object: Option<&str>,
) -> String {
    occurrence_key(
        subject,
        event_type,
        action_role,
        time,
        place,
        primary_object,
        None,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TypedTime;

    fn year(y: i32) -> TypedTime {
        TypedTime::Exact {
            year: y,
            month: None,
            day: None,
            surface: Some(y.to_string()),
        }
    }

    #[test]
    fn arrival_and_departure_differ() {
        let t = TypedTime::Exact {
            year: 1815,
            month: Some(3),
            day: Some(1),
            surface: Some("1815-03-01".into()),
        };
        let a = occurrence_key_for_event("S", "arrival", "arrived_in", &t, Some("Grenoble"), None);
        let b =
            occurrence_key_for_event("S", "departure", "departed_for", &t, Some("Grenoble"), None);
        assert_ne!(a, b);
    }

    #[test]
    fn two_battles_same_year_differ_by_object() {
        let t = year(1805);
        let a = occurrence_key_for_event(
            "S",
            "battle",
            "fought_at",
            &t,
            Some("Austerlitz"),
            Some("Battle of Austerlitz"),
        );
        let b = occurrence_key_for_event(
            "S",
            "battle",
            "fought_at",
            &t,
            Some("Ulm"),
            Some("Battle of Ulm"),
        );
        assert_ne!(a, b);
    }

    #[test]
    fn same_occurrence_stable() {
        let t = year(1815);
        let a = occurrence_key_for_event(
            "S",
            "battle",
            "fought_at",
            &t,
            Some("Waterloo"),
            Some("Battle of Waterloo"),
        );
        let b = occurrence_key_for_event(
            "S",
            "battle",
            "fought_at",
            &t,
            Some("Waterloo"),
            Some("Battle of Waterloo"),
        );
        assert_eq!(a, b);
    }

    #[test]
    fn two_extractors_same_fact_same_occurrence() {
        let t = year(1815);
        // Historically identical args — extractor must not appear in the key.
        let dense = occurrence_key_for_event(
            "Napoleon",
            "battle",
            "fought_at",
            &t,
            Some("Waterloo"),
            Some("Battle of Waterloo"),
        );
        let military = occurrence_key_for_event(
            "Napoleon",
            "battle",
            "fought_at",
            &t,
            Some("Waterloo"),
            Some("Battle of Waterloo"),
        );
        assert_eq!(dense, military);
        // Anti-pattern (old Lot E): extractor as primary_document splits the fact.
        let bad = occurrence_key(
            "Napoleon",
            "battle",
            "fought_at",
            &t,
            Some("Waterloo"),
            Some("Battle of Waterloo"),
            Some("dense_clause"),
            None,
        );
        assert_ne!(dense, bad);
    }
}
