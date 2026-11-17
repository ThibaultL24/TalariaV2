// crates/talaria-quality/src/attribution.rs
//! Subject attribution for events extracted from followed vs biography pages.

use crate::gates::{GateDecision, RejectionCode};
use crate::model::ParticipantRole;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributionMatch {
    DirectNameMatch,
    AliasMatch,
    TitleSubjectMatch,
    StructuredParticipantMatch,
    FollowedMilitaryAction,
    /// The page is about an action, but the passage does not show the subject acting.
    MentionedNotActing,
    CoreferenceMatch,
    /// The subject name is a comparison. Another person performs the action.
    Comparison,
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

fn is_agreement_type(event_type: &str) -> bool {
    matches!(event_type, "treaty" | "diplomatic")
}

fn is_role_bearing_action(event_type: &str) -> bool {
    is_military_action_type(event_type) || is_agreement_type(event_type)
}

/// The role the passage actually gives the subject. A nearby name is not a role.
pub fn demonstrated_role(quote: &str, subject: &str) -> Option<ParticipantRole> {
    let folded = fold_case(quote);
    let name = fold_case(subject);
    let token = name.split_whitespace().next().unwrap_or(name.as_str());
    if token.chars().count() < 4 || !contains_word(&folded, token) {
        return None;
    }
    if is_signatory(&folded, token) {
        return Some(ParticipantRole::Signatory);
    }
    if cue_before(&folded, token, &["command of", "commanded by", "under", "by"])
        || folded.contains(&format!("{token}'"))
    {
        return Some(ParticipantRole::Commander);
    }
    if cue_after(&folded, token, &["defeated", "fought", "attacked"]) {
        return Some(ParticipantRole::Participant);
    }
    if cue_before(&folded, token, &["addressed to", "wrote to"]) {
        return Some(ParticipantRole::Addressee);
    }
    None
}

fn is_signatory(hay: &str, name: &str) -> bool {
    let mentions_signing = hay.contains("signed") || hay.contains("signe") || hay.contains("truce");
    mentions_signing
        && (hay.contains(&format!("and {name}"))
            || hay.contains(&format!("{name} signed"))
            || hay.contains(&format!("signed by {name}"))
            || hay.contains(&format!("signe par {name}")))
}

fn cue_before(hay: &str, name: &str, cues: &[&str]) -> bool {
    let Some(name_at) = word_at(hay, name) else {
        return false;
    };
    let window_start = name_at.saturating_sub(24);
    let window = &hay[window_start..name_at];
    cues.iter().any(|cue| window.contains(cue))
}

fn cue_after(hay: &str, name: &str, cues: &[&str]) -> bool {
    let Some(name_at) = word_at(hay, name) else {
        return false;
    };
    let after = &hay[name_at + name.len()..];
    let window = &after[..after.len().min(24)];
    cues.iter().any(|cue| window.contains(cue))
}

fn word_at(hay: &str, word: &str) -> Option<usize> {
    hay.match_indices(word).find_map(|(index, _)| {
        contains_word(&hay[index..], word).then_some(index)
    })
}

fn contains_word(hay: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    hay.match_indices(word).any(|(index, _)| {
        let before = hay[..index]
            .chars()
            .next_back()
            .map(|c| !c.is_alphabetic())
            .unwrap_or(true);
        let after = hay[index + word.len()..]
            .chars()
            .next()
            .map(|c| !c.is_alphabetic())
            .unwrap_or(true);
        before && after
    })
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
    ["he ", "she ", "the emperor ", "l'empereur "]
        .iter()
        .any(|prefix| folded.starts_with(prefix))
}

pub fn classify_attribution(input: &AttributionInput<'_>) -> AttributionMatch {
    if subject_is_only_a_comparison(input.quote, input.subject) {
        return AttributionMatch::Comparison;
    }
    if input.structured_source {
        return AttributionMatch::StructuredParticipantMatch;
    }

    if input.military_subject
        && input.from_followed_page
        && is_role_bearing_action(input.event_type)
    {
        return match demonstrated_role(input.quote, input.subject) {
            Some(_) if is_military_action_type(input.event_type) => {
                AttributionMatch::FollowedMilitaryAction
            }
            Some(_) => AttributionMatch::DirectNameMatch,
            None => AttributionMatch::MentionedNotActing,
        };
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

fn subject_is_only_a_comparison(quote: &str, subject: &str) -> bool {
    let surname = subject.split_whitespace().last().unwrap_or(subject).trim();
    if surname.chars().count() < 4 {
        return false;
    }
    let folded = fold_case(quote);
    let name = fold_case(surname);
    let patterns = [
        format!("the {name} of "),
        format!("a {name} of "),
        format!("le {name} de "),
        format!("un {name} de "),
        format!("like {name}"),
        format!("comme {name}"),
        format!("compared to {name}"),
        format!("surnomme le {name}"),
        format!("called the {name}"),
        format!("known as the {name}"),
        format!("considered by some to be the {name}"),
    ];
    if !patterns
        .iter()
        .any(|pattern| folded.contains(pattern.as_str()))
    {
        return false;
    }
    let direct = [
        format!("{name} defeated"),
        format!("{name} fought"),
        format!("{name} was born"),
        format!("{name} married"),
        format!("{name} signed"),
        format!("{name} commanded"),
    ];
    !direct.iter().any(|cue| folded.contains(cue.as_str()))
}

pub fn attribution_gate_decision(m: AttributionMatch) -> GateDecision {
    match m {
        AttributionMatch::MentionedNotActing | AttributionMatch::CoreferenceMatch => {
            GateDecision::NeedsReview
        }
        AttributionMatch::Comparison => {
            GateDecision::Reject(vec![RejectionCode::SubjectUsedAsComparison])
        }
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
    fn an_indirect_action_that_names_the_subject_stays_attributable() {
        let m = classify_attribution(&AttributionInput {
            subject: "Napoleon",
            aliases: &[],
            quote: "The French army under Napoleon defeated the Austrians at Lodi.",
            page_title: "Battle of Lodi",
            event_type: "battle",
            from_followed_page: true,
            structured_source: false,
            role_supported_by_evidence: true,
            military_subject: true,
        });
        assert_eq!(m, AttributionMatch::FollowedMilitaryAction);
        assert!(auto_accept_attribution(m));
        let unnamed = classify_attribution(&AttributionInput {
            subject: "Napoleon",
            aliases: &[],
            quote: "The French army defeated the Austrians at Lodi.",
            page_title: "Battle of Lodi",
            event_type: "diplomatic",
            from_followed_page: true,
            structured_source: false,
            role_supported_by_evidence: false,
            military_subject: false,
        });
        assert_eq!(unnamed, AttributionMatch::Unattributed);
        assert!(!auto_accept_attribution(unnamed));
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
    fn a_followed_battle_without_the_subject_role_stays_in_review() {
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
        assert_eq!(m, AttributionMatch::MentionedNotActing);
        assert!(!auto_accept_attribution(m));
        assert!(matches!(
            attribution_gate_decision(m),
            GateDecision::NeedsReview
        ));
    }

    #[test]
    fn arcole_names_his_army_ceva_names_augereau_and_tolentino_names_murat() {
        let arcole = classify_attribution(&AttributionInput {
            subject: "Napoleon",
            aliases: &["Napoleon Bonaparte"],
            quote: "The battle saw a bold maneuver by Napoleon Bonaparte's French Army of Italy.",
            page_title: "Battle of Arcole",
            event_type: "battle",
            from_followed_page: true,
            structured_source: false,
            role_supported_by_evidence: true,
            military_subject: true,
        });
        assert_eq!(arcole, AttributionMatch::FollowedMilitaryAction);
        assert_eq!(
            demonstrated_role(
                "The battle saw a bold maneuver by Napoleon Bonaparte's French Army of Italy.",
                "Napoleon"
            ),
            Some(ParticipantRole::Commander)
        );

        let ceva = classify_attribution(&AttributionInput {
            subject: "Napoleon",
            aliases: &[],
            quote: "The Battle of Ceva was fought by the French under General Pierre Augereau.",
            page_title: "Battle of Ceva",
            event_type: "battle",
            from_followed_page: true,
            structured_source: false,
            role_supported_by_evidence: false,
            military_subject: true,
        });
        assert_eq!(ceva, AttributionMatch::MentionedNotActing);

        let tolentino = classify_attribution(&AttributionInput {
            subject: "Napoleon",
            aliases: &[],
            quote: "The Battle of Tolentino was fought by the Napoleonic King of Naples Joachim Murat.",
            page_title: "Battle of Tolentino",
            event_type: "battle",
            from_followed_page: true,
            structured_source: false,
            role_supported_by_evidence: false,
            military_subject: true,
        });
        assert_eq!(tolentino, AttributionMatch::MentionedNotActing);
        assert_eq!(
            demonstrated_role(
                "The Battle of Tolentino was fought by the Napoleonic King of Naples Joachim Murat.",
                "Napoleon"
            ),
            None
        );
    }

    #[test]
    fn a_signed_truce_names_the_signatory_not_every_mentioned_treaty() {
        let cherasco = demonstrated_role(
            "The Armistice of Cherasco was a truce signed at Cherasco on 28 April 1796 between Victor Amadeus III of Sardinia and Napoleon Bonaparte.",
            "Napoleon",
        );
        assert_eq!(cherasco, Some(ParticipantRole::Signatory));
        let campo = classify_attribution(&AttributionInput {
            subject: "Napoleon",
            aliases: &[],
            quote: "The Treaty of Campo Formio was a peace treaty signed on 17 October 1797 between the French Republic and Austria.",
            page_title: "Treaty of Campo Formio",
            event_type: "treaty",
            from_followed_page: true,
            structured_source: false,
            role_supported_by_evidence: false,
            military_subject: true,
        });
        assert_eq!(campo, AttributionMatch::MentionedNotActing);
    }

    #[test]
    fn a_comparison_with_napoleon_is_not_his_action() {
        let quote = "Meanwhile, Simón Bolívar, who is considered by some to be the Napoleon of South America, entered Chuquisaca.";
        let m = classify_attribution(&AttributionInput {
            subject: "Napoleon",
            aliases: &["Napoleon Bonaparte"],
            quote,
            page_title: "Napoleon",
            event_type: "arrival",
            from_followed_page: false,
            structured_source: false,
            role_supported_by_evidence: true,
            military_subject: true,
        });
        assert_eq!(m, AttributionMatch::Comparison);
        assert!(matches!(
            attribution_gate_decision(m),
            GateDecision::Reject(codes) if codes == vec![RejectionCode::SubjectUsedAsComparison]
        ));
    }

    #[test]
    fn napoleon_as_the_actor_stays_attributed() {
        let m = classify_attribution(&AttributionInput {
            subject: "Napoleon",
            aliases: &[],
            quote: "Napoleon defeated the Austrian army at the Battle of Lodi on 10 May 1796.",
            page_title: "Napoleon",
            event_type: "battle",
            from_followed_page: false,
            structured_source: false,
            role_supported_by_evidence: true,
            military_subject: true,
        });
        assert_eq!(m, AttributionMatch::TitleSubjectMatch);
        assert!(auto_accept_attribution(m));
    }
}
