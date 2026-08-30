// crates/talaria-quality/src/attribution.rs
//! Subject attribution for events extracted from followed vs biography pages.

use crate::gates::{GateDecision, RejectionCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributionMatch {
    DirectNameMatch,
    AliasMatch,
    TitleSubjectMatch,
    StructuredParticipantMatch,
    FollowedMilitaryAction,
    CoreferenceMatch,
    Unattributed,
}

pub struct AttributionInput<'a> {
    pub subject: &'a str,
    pub aliases: &'a [&'a str],
    pub quote: &'a str,
    pub page_title: &'a str,
    pub event_type: &'a str,
    pub from_followed_page: bool,
    pub structured_source: bool,
    pub role_supported_by_evidence: bool,
    pub military_subject: bool,
}

fn is_military_action_type(event_type: &str) -> bool {
    matches!(
        event_type,
        "battle" | "siege" | "military_campaign" | "surrender" | "retreat"
    )
}

fn fold_case(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' | 'ā' => 'a',
            'è' | 'é' | 'ê' | 'ë' | 'ē' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'ī' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' | 'ø' | 'ō' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'ū' => 'u',
            'ý' | 'ÿ' => 'y',
            'ñ' => 'n',
            'ç' => 'c',
            'ł' => 'l',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            _ => c,
        })
        .collect()
}

fn contains_folded(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    fold_case(haystack).contains(&fold_case(needle))
}

fn titles_equal(a: &str, b: &str) -> bool {
    fold_case(a) == fold_case(b)
}

fn starts_with_coreference(quote: &str) -> bool {
    let trimmed = quote.trim_start();
    let folded = fold_case(trimmed);
    [
        "he ",
        "she ",
        "the emperor ",
        "l'empereur ",
    ]
    .iter()
    .any(|prefix| folded.starts_with(prefix))
}

pub fn classify_attribution(input: &AttributionInput<'_>) -> AttributionMatch {
    if input.structured_source {
        return AttributionMatch::StructuredParticipantMatch;
    }

    if input.military_subject
        && input.from_followed_page
        && is_military_action_type(input.event_type)
    {
        return AttributionMatch::FollowedMilitaryAction;
    }

    if !input.from_followed_page {
        return AttributionMatch::TitleSubjectMatch;
    }

    if titles_equal(input.page_title, input.subject) {
        return AttributionMatch::TitleSubjectMatch;
    }

    let name_in_quote = contains_folded(input.quote, input.subject);
    let alias_in_quote = input
        .aliases
        .iter()
        .any(|alias| contains_folded(input.quote, alias));

    if name_in_quote || alias_in_quote {
        if !input.role_supported_by_evidence {
            return AttributionMatch::Unattributed;
        }
        if name_in_quote {
            return AttributionMatch::DirectNameMatch;
        }
        return AttributionMatch::AliasMatch;
    }

    if starts_with_coreference(input.quote) {
        return AttributionMatch::CoreferenceMatch;
    }

    AttributionMatch::Unattributed
}

pub fn auto_accept_attribution(m: AttributionMatch) -> bool {
    matches!(
        m,
        AttributionMatch::DirectNameMatch
            | AttributionMatch::AliasMatch
            | AttributionMatch::TitleSubjectMatch
            | AttributionMatch::StructuredParticipantMatch
            | AttributionMatch::FollowedMilitaryAction
    )
}

pub fn attribution_gate_decision(m: AttributionMatch) -> GateDecision {
    match m {
        AttributionMatch::CoreferenceMatch => GateDecision::NeedsReview,
        AttributionMatch::Unattributed => {
            GateDecision::Reject(vec![RejectionCode::SubjectNotAttributed])
        }
        _ => GateDecision::Accept,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn followed_battle_page_without_role_evidence_is_unattributed() {
        let m = classify_attribution(&AttributionInput {
            subject: "Victor Hugo",
            aliases: &["Hugo"],
            quote: "The Battle of Plevna was fought in 1877.",
            page_title: "Siege of Plevna",
            event_type: "battle",
            from_followed_page: true,
            structured_source: false,
            role_supported_by_evidence: false,
            military_subject: false,
        });
        assert_eq!(m, AttributionMatch::Unattributed);
        assert!(!auto_accept_attribution(m));
    }

    #[test]
    fn biography_page_is_title_subject_match() {
        let m = classify_attribution(&AttributionInput {
            subject: "Victor Hugo",
            aliases: &[],
            quote: "He was born in Besançon.",
            page_title: "Victor Hugo",
            event_type: "birth",
            from_followed_page: false,
            structured_source: false,
            role_supported_by_evidence: true,
            military_subject: false,
        });
        assert_eq!(m, AttributionMatch::TitleSubjectMatch);
        assert!(auto_accept_attribution(m));
    }

    #[test]
    fn coreference_on_followed_page_is_not_auto_accept() {
        let m = classify_attribution(&AttributionInput {
            subject: "Napoleon",
            aliases: &[],
            quote: "He then returned to Paris.",
            page_title: "War of the Sixth Coalition",
            event_type: "travel",
            from_followed_page: true,
            structured_source: false,
            role_supported_by_evidence: true,
            military_subject: true,
        });
        assert_eq!(m, AttributionMatch::CoreferenceMatch);
        assert!(!auto_accept_attribution(m));
    }

    #[test]
    fn wdqs_structured_is_structured_participant() {
        let m = classify_attribution(&AttributionInput {
            subject: "Napoleon",
            aliases: &[],
            quote: "",
            page_title: "WDQS events for Q517",
            event_type: "battle",
            from_followed_page: true,
            structured_source: true,
            role_supported_by_evidence: true,
            military_subject: true,
        });
        assert_eq!(m, AttributionMatch::StructuredParticipantMatch);
    }

    #[test]
    fn military_followed_battle_page_is_auto_accept() {
        let m = classify_attribution(&AttributionInput {
            subject: "Napoleon",
            aliases: &[],
            quote: "The battle was fought on 2 December 1805 near Austerlitz.",
            page_title: "Battle of Austerlitz",
            event_type: "battle",
            from_followed_page: true,
            structured_source: false,
            role_supported_by_evidence: false,
            military_subject: true,
        });
        assert_eq!(m, AttributionMatch::FollowedMilitaryAction);
        assert!(auto_accept_attribution(m));
    }
}
