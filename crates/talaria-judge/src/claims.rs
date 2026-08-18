// crates/talaria-judge/src/claims.rs
//! Rule-based claim kind classification (soft-accept; no geo/time required).

#[derive(Debug, Clone, PartialEq)]
pub struct ClaimClass {
    pub kind: &'static str,
    pub epistemic_status: &'static str,
    pub relation_to_subject: &'static str,
    pub confidence: f64,
}

pub fn classify_claim_text(text: &str) -> ClaimClass {
    let lower = text.to_ascii_lowercase();

    if has_any(
        &lower,
        &[
            "controversial",
            "disputed",
            "scandal",
            "accused of",
            "allegation",
            "denied that",
        ],
    ) {
        return ClaimClass {
            kind: "controversy",
            epistemic_status: "disputed",
            relation_to_subject: "direct",
            confidence: 0.55,
        };
    }

    if has_any(
        &lower,
        &[
            "legend has it",
            "according to legend",
            "anecdote",
            "once told",
            "is said to have",
            "it is said that",
            "it is said ",
            "reportedly",
            "apocryphal",
            "the story goes",
            "popular story",
            "reputed to have",
            "tradition holds",
            "folklore",
            "according to a popular",
        ],
    ) {
        return ClaimClass {
            kind: "anecdote",
            epistemic_status: "attested",
            relation_to_subject: "direct",
            confidence: 0.5,
        };
    }

    if has_any(
        &lower,
        &[
            "historians debate",
            "critics argue",
            "supporters claim",
            "on the other hand",
            "however, some",
        ],
    ) {
        return ClaimClass {
            kind: "debate_stance",
            epistemic_status: "contested",
            relation_to_subject: "historiography",
            confidence: 0.5,
        };
    }

    if has_any(
        &lower,
        &[
            "theory",
            "hypothes",
            "believed that",
            "argued that",
            "proposed that",
            "suggested that",
        ],
    ) {
        return ClaimClass {
            kind: "theory",
            epistemic_status: "hypothesized",
            relation_to_subject: "indirect",
            confidence: 0.55,
        };
    }

    if has_any(
        &lower,
        &[" wrote ", " said ", " according to ", " quoted ", " remarked "],
    ) {
        return ClaimClass {
            kind: "attribution",
            epistemic_status: "attested",
            relation_to_subject: "direct",
            confidence: 0.6,
        };
    }

    if looks_like_life_event(&lower) {
        return ClaimClass {
            kind: "life_event",
            epistemic_status: "attested",
            relation_to_subject: "direct",
            confidence: 0.65,
        };
    }

    if has_any(
        &lower,
        &["during", "while", "at the time", "in this period", "meanwhile"],
    ) {
        return ClaimClass {
            kind: "context",
            epistemic_status: "attested",
            relation_to_subject: "indirect",
            confidence: 0.45,
        };
    }

    ClaimClass {
        kind: "fact",
        epistemic_status: "attested",
        relation_to_subject: "direct",
        confidence: 0.4,
    }
}

fn looks_like_life_event(lower: &str) -> bool {
    let verbs = [
        "was born",
        "born in",
        "died",
        "married",
        "crowned",
        "elected",
        "appointed",
        "abdicated",
        "graduated",
        "founded",
        "published",
        "moved to",
        "fled",
        "executed",
        "assassinated",
    ];
    let has_verb = has_any(lower, &verbs);
    let has_year = lower.chars().any(|c| c.is_ascii_digit())
        && lower.contains(|c: char| c.is_ascii_digit())
        && (lower.contains("1") || lower.contains("2"));
    // crude year cue: four digits
    let has_four_digits = lower
        .split(|c: char| !c.is_ascii_digit())
        .any(|part| part.len() == 4);
    has_verb && (has_year || has_four_digits)
}

fn has_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_birth_as_life_event() {
        let c = classify_claim_text("Napoleon Bonaparte was born in 1769 in Ajaccio.");
        assert_eq!(c.kind, "life_event");
    }

    #[test]
    fn classifies_anecdote() {
        let c = classify_claim_text("According to legend, he slept only four hours a night.");
        assert_eq!(c.kind, "anecdote");
    }

    #[test]
    fn classifies_controversy() {
        let c = classify_claim_text("The claim remains controversial among historians.");
        assert_eq!(c.kind, "controversy");
    }
}
