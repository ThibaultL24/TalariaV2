// crates/talaria-quality/src/assertion.rs
//! Competing sourced readings of the same historical question.
//! Canonical events are projections; abstain when places disagree.

use crate::fingerprint::normalize_surface;
use crate::occurrence::occurrence_key_for_event;
use crate::TypedTime;

pub const EXTRACTOR_EPISTEMIC_STATUS: &str = "attested";
pub const ABSTAIN_COMPETING_PLACE: &str = "competing_place";

/// Occurrence identity with place stripped — groups competing location readings.
pub fn occurrence_stem_for_event(
    subject: &str,
    event_type: &str,
    action_role: &str,
    time: &TypedTime,
    primary_object: Option<&str>,
) -> String {
    occurrence_key_for_event(
        subject,
        event_type,
        action_role,
        time,
        None,
        primary_object,
    )
}

/// Two non-empty, distinct places at the same stem are a contradiction.
/// A missing place is incomplete evidence, not a contradiction.
pub fn competing_places(incoming: Option<&str>, existing: &[Option<&str>]) -> Option<Vec<String>> {
    let incoming = normalize_surface(incoming.unwrap_or(""));
    if incoming.is_empty() {
        return None;
    }
    let mut rivals: Vec<String> = existing
        .iter()
        .map(|p| normalize_surface(p.unwrap_or("")))
        .filter(|n| !n.is_empty() && *n != incoming)
        .collect();
    if rivals.is_empty() {
        return None;
    }
    rivals.push(incoming);
    rivals.sort();
    rivals.dedup();
    Some(rivals)
}

#[cfg(test)]
mod tests {
    use super::{competing_places, occurrence_stem_for_event};
    use crate::occurrence::occurrence_key_for_event;
    use crate::TypedTime;

    fn may_18() -> TypedTime {
        TypedTime::Exact {
            year: 1814,
            month: Some(5),
            day: Some(18),
            surface: Some("1814-05-18".into()),
        }
    }

    fn year(y: i32) -> TypedTime {
        TypedTime::Exact {
            year: y,
            month: None,
            day: None,
            surface: Some(y.to_string()),
        }
    }

    #[test]
    fn paris_and_fontainebleau_same_day_share_stem() {
        let t = may_18();
        let paris = occurrence_stem_for_event("Napoleon", "residence", "located_at", &t, None);
        let font = occurrence_stem_for_event("Napoleon", "residence", "located_at", &t, None);
        assert_eq!(paris, font);
        let occ_paris = occurrence_key_for_event(
            "Napoleon",
            "residence",
            "located_at",
            &t,
            Some("Paris"),
            None,
        );
        let occ_font = occurrence_key_for_event(
            "Napoleon",
            "residence",
            "located_at",
            &t,
            Some("Fontainebleau"),
            None,
        );
        assert_ne!(occ_paris, occ_font);
        assert_ne!(paris, occ_paris);
    }

    #[test]
    fn competing_places_paris_vs_fontainebleau() {
        let places = competing_places(Some("Paris"), &[Some("Fontainebleau")]);
        assert_eq!(
            places,
            Some(vec!["fontainebleau".into(), "paris".into()])
        );
    }

    #[test]
    fn same_place_does_not_compete() {
        assert_eq!(competing_places(Some("Paris"), &[Some("paris")]), None);
    }

    #[test]
    fn missing_place_is_incomplete_not_a_contradiction() {
        assert_eq!(competing_places(Some("Paris"), &[None]), None);
        assert_eq!(competing_places(None, &[Some("Paris")]), None);
    }

    #[test]
    fn extractors_never_write_established() {
        assert_eq!(super::EXTRACTOR_EPISTEMIC_STATUS, "attested");
    }

    #[test]
    fn distinct_battle_objects_have_distinct_stems() {
        let t = year(1805);
        let a = occurrence_stem_for_event(
            "Napoleon",
            "battle",
            "fought_at",
            &t,
            Some("Battle of Austerlitz"),
        );
        let b = occurrence_stem_for_event("Napoleon", "battle", "fought_at", &t, Some("Battle of Ulm"));
        assert_ne!(a, b);
    }
}
