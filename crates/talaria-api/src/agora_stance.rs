// crates/talaria-api/src/agora_stance.rs
//! Agora theory stances: believe = triple vault, dispute = counter-triple.
//! Frontend never receives calldata.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StanceKind {
    Believe,
    Dispute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StanceBlock {
    NotTheory,
    NotOnChain,
    LiveDisabled,
    MinSharesForbidden,
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

pub fn is_agora_theory(claim_kind: &str) -> bool {
    claim_kind.trim().eq_ignore_ascii_case("theory")
}

pub fn stance_block(
    claim_kind: &str,
    publication_status: Option<&str>,
    triple_term_id: Option<&str>,
    min_shares: u128,
) -> Option<StanceBlock> {
    if !is_agora_theory(claim_kind) {
        return Some(StanceBlock::NotTheory);
    }
    let on_chain = publication_status == Some("published")
        && triple_term_id.map(|id| !id.trim().is_empty()).unwrap_or(false);
    if !on_chain {
        return Some(StanceBlock::NotOnChain);
    }
    if min_shares == 0 {
        return Some(StanceBlock::MinSharesForbidden);
    }
    Some(StanceBlock::LiveDisabled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_theory_kind_is_actionable() {
        assert!(is_agora_theory("theory"));
        assert!(is_agora_theory("Theory"));
        assert!(!is_agora_theory("controversy"));
        assert!(!is_agora_theory("historical_fact"));
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
    fn unpublished_theory_is_not_on_chain() {
        assert_eq!(
            stance_block("theory", None, None, 1),
            Some(StanceBlock::NotOnChain)
        );
        assert_eq!(
            stance_block("theory", Some("planned"), None, 1),
            Some(StanceBlock::NotOnChain)
        );
    }

    #[test]
    fn published_theory_still_blocks_live_deposit() {
        assert_eq!(
            stance_block("theory", Some("published"), Some("0xabc"), 1),
            Some(StanceBlock::LiveDisabled)
        );
    }

    #[test]
    fn min_shares_zero_is_rejected() {
        assert_eq!(
            stance_block("theory", Some("published"), Some("0xabc"), 0),
            Some(StanceBlock::MinSharesForbidden)
        );
    }

    #[test]
    fn controversy_is_not_theory() {
        assert_eq!(
            stance_block("controversy", Some("published"), Some("0xabc"), 1),
            Some(StanceBlock::NotTheory)
        );
    }
}
