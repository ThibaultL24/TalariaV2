// crates/talaria-quality/src/occurrence.rs
//! Occurrence keys — finer than event_fingerprint year+place merge.

use sha2::{Digest, Sha256};

use crate::fingerprint::normalize_surface;
use crate::model::TypedTime;

/// Structured key distinguishing historically distinct occurrences.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TypedTime;

    #[test]
    fn arrival_and_departure_differ() {
        let t = TypedTime::Exact {
            year: 1815,
            month: Some(3),
            day: Some(1),
            surface: Some("1815-03-01".into()),
        };
        let a = occurrence_key("S", "arrival", "arrived_in", &t, Some("Grenoble"), None, None, None);
        let b = occurrence_key("S", "departure", "departed_for", &t, Some("Grenoble"), None, None, None);
        assert_ne!(a, b);
    }

    #[test]
    fn two_battles_same_year_differ_by_object() {
        let t = TypedTime::Exact {
            year: 1805,
            month: None,
            day: None,
            surface: Some("1805".into()),
        };
        let a = occurrence_key(
            "S",
            "battle",
            "fought_at",
            &t,
            Some("Austerlitz"),
            Some("Battle of Austerlitz"),
            None,
            None,
        );
        let b = occurrence_key(
            "S",
            "battle",
            "fought_at",
            &t,
            Some("Ulm"),
            Some("Battle of Ulm"),
            None,
            None,
        );
        assert_ne!(a, b);
    }

    #[test]
    fn same_occurrence_stable() {
        let t = TypedTime::Exact {
            year: 1815,
            month: None,
            day: None,
            surface: Some("1815".into()),
        };
        let a = occurrence_key("S", "battle", "fought_at", &t, Some("Waterloo"), Some("Battle of Waterloo"), None, None);
        let b = occurrence_key("S", "battle", "fought_at", &t, Some("Waterloo"), Some("Battle of Waterloo"), None, None);
        assert_eq!(a, b);
    }
}
