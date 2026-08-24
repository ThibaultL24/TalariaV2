// crates/talaria-wikidata/src/promote.rs
//! Closed-list promotion of Wikidata PIDs to EventCandidate type/predicate.

pub fn promote_event(
    pid: &str,
    has_date: bool,
    subject_is_participant: bool,
) -> Option<(&'static str, &'static str)> {
    match pid {
        "P569" if has_date => Some(("birth", "born_in")),
        "P570" if has_date => Some(("death", "died_in")),
        "P793" if has_date => Some(("notable_event", "occurred")),
        "P39" if has_date => Some(("office", "held_office")),
        "P26" if has_date => Some(("marriage", "married")),
        "P69" if has_date => Some(("education", "studied_at")),
        "P551" if has_date => Some(("residence", "resided_in")),
        "P607" | "P1344" if has_date && subject_is_participant => Some(("battle", "fought_at")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p551_without_date_is_claim() {
        assert!(promote_event("P551", false, true).is_none());
    }

    #[test]
    fn p551_with_date_is_residence() {
        assert_eq!(
            promote_event("P551", true, true),
            Some(("residence", "resided_in"))
        );
    }

    #[test]
    fn p106_never_event() {
        assert!(promote_event("P106", true, true).is_none());
    }

    #[test]
    fn p569_is_birth() {
        assert_eq!(
            promote_event("P569", true, true),
            Some(("birth", "born_in"))
        );
    }

    #[test]
    fn p570_with_date_is_death() {
        assert_eq!(
            promote_event("P570", true, true),
            Some(("death", "died_in"))
        );
    }

    #[test]
    fn p570_without_date_is_claim() {
        assert!(promote_event("P570", false, true).is_none());
    }

    #[test]
    fn p793_with_date_is_notable_event() {
        assert_eq!(
            promote_event("P793", true, true),
            Some(("notable_event", "occurred"))
        );
    }

    #[test]
    fn p39_with_date_is_office() {
        assert_eq!(
            promote_event("P39", true, true),
            Some(("office", "held_office"))
        );
    }

    #[test]
    fn p26_with_date_is_marriage() {
        assert_eq!(
            promote_event("P26", true, true),
            Some(("marriage", "married"))
        );
    }

    #[test]
    fn p69_with_date_is_education() {
        assert_eq!(
            promote_event("P69", true, true),
            Some(("education", "studied_at"))
        );
    }

    #[test]
    fn p19_alone_is_claim() {
        assert!(promote_event("P19", true, true).is_none());
    }

    #[test]
    fn p607_needs_date_and_participant() {
        assert!(promote_event("P607", true, false).is_none());
        assert!(promote_event("P607", false, true).is_none());
        assert_eq!(
            promote_event("P607", true, true),
            Some(("battle", "fought_at"))
        );
    }

    #[test]
    fn p1344_needs_date_and_participant() {
        assert_eq!(
            promote_event("P1344", true, true),
            Some(("battle", "fought_at"))
        );
    }

    #[test]
    fn other_dated_property_is_claim() {
        assert!(promote_event("P937", true, true).is_none());
        assert!(promote_event("P27", true, true).is_none());
    }
}
