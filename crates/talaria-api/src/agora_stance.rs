// crates/talaria-api/src/agora_stance.rs
//! Intuition stances: believe = triple vault, dispute = counter-triple.
//! Frontend never receives calldata.
//! Eligible Agora claim kinds: theory, controversy, and interpretive claims.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StanceKind {
    Believe,
    Dispute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StanceBlock {
    NotTheory,
    NotOnChain,
    ModeledPending,
    /// Published on-chain but live settle/deposit is disabled in this process.
    LiveDisabled,
    /// Published + live allowed — previewDeposit / optional execute may proceed.
    Ready,
    MinSharesForbidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StanceTargetKind {
    Person,
    Event,
    Claim,
    Source,
}

impl StanceTargetKind {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "person" | "entity" => Some(Self::Person),
            "event" => Some(Self::Event),
            "claim" | "theory" | "controversy" | "interpretation" => Some(Self::Claim),
            "source" | "document" | "evidence" => Some(Self::Source),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Event => "event",
            Self::Claim => "claim",
            Self::Source => "source",
        }
    }
}

impl StanceKind {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "believe" | "for" | "yes" => Some(Self::Believe),
            "dispute" | "against" | "no" => Some(Self::Dispute),
            _ => None,
        }
    }

    pub fn vault_role(self) -> &'static str {
        match self {
            Self::Believe => "triple",
            Self::Dispute => "counter_triple",
        }
    }
}

/// Soft-claim kinds that may carry an Intuition stance (export surface).
pub fn is_stance_claim(claim_kind: &str) -> bool {
    matches!(
        claim_kind.trim().to_ascii_lowercase().as_str(),
        "theory" | "controversy" | "debate_stance"
    )
}

/// Backward-compatible alias — theories remain the primary publish surface.
pub fn is_agora_theory(claim_kind: &str) -> bool {
    claim_kind.trim().eq_ignore_ascii_case("theory")
}

fn is_modeled_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "pending" | "planned" | "modeled" | "exported"
    )
}

/// Claim-level stance gate. Eligible kinds without an on-chain row are modeled.
/// `live_allowed` comes from `INTUITION_ALLOW_LIVE` (operator opt-in).
pub fn stance_block(
    claim_kind: &str,
    publication_status: Option<&str>,
    triple_term_id: Option<&str>,
    min_shares: u128,
    live_allowed: bool,
) -> Option<StanceBlock> {
    if !is_stance_claim(claim_kind) {
        return Some(StanceBlock::NotTheory);
    }
    let on_chain = publication_status == Some("published")
        && triple_term_id.map(|id| !id.trim().is_empty()).unwrap_or(false);
    if !on_chain {
        if publication_status.map(is_modeled_status).unwrap_or(true) {
            // No publication row → still modeled so Believe/Dispute is usable in the demo.
            return Some(StanceBlock::ModeledPending);
        }
        return Some(StanceBlock::NotOnChain);
    }
    if min_shares == 0 {
        return Some(StanceBlock::MinSharesForbidden);
    }
    if live_allowed {
        Some(StanceBlock::Ready)
    } else {
        Some(StanceBlock::LiveDisabled)
    }
}

/// Person / event / source targets are always modeled until a dedicated publish path exists.
pub fn modeled_target_block(min_shares: u128) -> StanceBlock {
    if min_shares == 0 {
        StanceBlock::MinSharesForbidden
    } else {
        StanceBlock::ModeledPending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stance_claims_cover_doctrine_targets() {
        assert!(is_stance_claim("theory"));
        assert!(is_stance_claim("controversy"));
        assert!(is_stance_claim("debate_stance"));
        assert!(!is_stance_claim("attribution"));
        assert!(!is_stance_claim("anecdote"));
        assert!(!is_stance_claim("life_event"));
        assert!(!is_stance_claim("fact"));
    }

    #[test]
    fn believe_targets_triple_dispute_targets_counter() {
        assert_eq!(StanceKind::parse("believe"), Some(StanceKind::Believe));
        assert_eq!(StanceKind::parse("dispute"), Some(StanceKind::Dispute));
        assert_eq!(StanceKind::Believe.vault_role(), "triple");
        assert_eq!(StanceKind::Dispute.vault_role(), "counter_triple");
        assert!(StanceKind::parse("maybe").is_none());
    }

    #[test]
    fn eligible_claim_without_publication_is_modeled() {
        assert_eq!(
            stance_block("theory", None, None, 1, false),
            Some(StanceBlock::ModeledPending)
        );
        assert_eq!(
            stance_block("controversy", None, None, 1, false),
            Some(StanceBlock::ModeledPending)
        );
    }

    #[test]
    fn pending_or_planned_publication_is_modeled() {
        assert_eq!(
            stance_block("theory", Some("pending"), None, 1, false),
            Some(StanceBlock::ModeledPending)
        );
        assert_eq!(
            stance_block("theory", Some("planned"), None, 1, false),
            Some(StanceBlock::ModeledPending)
        );
    }

    #[test]
    fn published_without_live_flag_is_disabled() {
        assert_eq!(
            stance_block("theory", Some("published"), Some("0xabc"), 1, false),
            Some(StanceBlock::LiveDisabled)
        );
    }

    #[test]
    fn published_with_live_flag_is_ready() {
        assert_eq!(
            stance_block("theory", Some("published"), Some("0xabc"), 1, true),
            Some(StanceBlock::Ready)
        );
    }

    #[test]
    fn min_shares_zero_is_rejected() {
        assert_eq!(
            stance_block("theory", Some("published"), Some("0xabc"), 0, true),
            Some(StanceBlock::MinSharesForbidden)
        );
    }

    #[test]
    fn person_event_source_parse() {
        assert_eq!(StanceTargetKind::parse("person"), Some(StanceTargetKind::Person));
        assert_eq!(StanceTargetKind::parse("event"), Some(StanceTargetKind::Event));
        assert_eq!(StanceTargetKind::parse("source"), Some(StanceTargetKind::Source));
        assert_eq!(modeled_target_block(1), StanceBlock::ModeledPending);
    }
}
