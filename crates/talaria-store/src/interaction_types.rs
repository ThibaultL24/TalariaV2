// crates/talaria-store/src/interaction_types.rs
//! Typed interaction vocabulary. Backend matrix is the source of truth.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionAction {
    Interest,
    Follow,
    Save,
    Support,
    Dispute,
    Uncertain,
    Useful,
    Credible,
    NotCredible,
    WantToVisit,
    Visited,
    /// Reserved: Visit itinerary UUID does not exist yet (PR8).
    Recommend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionTargetType {
    Person,
    Claim,
    Source,
    Place,
    Event,
    /// Reserved: Visit has no durable route id (PR8).
    Route,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionVisibility {
    Private,
    Public,
}

impl InteractionAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Interest => "interest",
            Self::Follow => "follow",
            Self::Save => "save",
            Self::Support => "support",
            Self::Dispute => "dispute",
            Self::Uncertain => "uncertain",
            Self::Useful => "useful",
            Self::Credible => "credible",
            Self::NotCredible => "not_credible",
            Self::WantToVisit => "want_to_visit",
            Self::Visited => "visited",
            Self::Recommend => "recommend",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(raw.to_string())).ok()
    }

    pub fn is_epistemic_stance(self) -> bool {
        matches!(self, Self::Support | Self::Dispute | Self::Uncertain)
    }
}

impl InteractionTargetType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Claim => "claim",
            Self::Source => "source",
            Self::Place => "place",
            Self::Event => "event",
            Self::Route => "route",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(raw.to_string())).ok()
    }

    /// PR3 persisted targets. `route` is not supported.
    pub fn is_supported(self) -> bool {
        !matches!(self, Self::Route)
    }
}

impl InteractionVisibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Public => "public",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(raw.to_string())).ok()
    }
}

/// Default when the client omits `visibility`.
///
/// | action | default |
/// |---|---|
/// | save, follow, interest, visited, want_to_visit | private |
/// | support, dispute, uncertain, useful, credible, not_credible, recommend | public |
pub fn default_visibility(action: InteractionAction) -> InteractionVisibility {
    match action {
        InteractionAction::Save
        | InteractionAction::Follow
        | InteractionAction::Interest
        | InteractionAction::Visited
        | InteractionAction::WantToVisit => InteractionVisibility::Private,
        InteractionAction::Support
        | InteractionAction::Dispute
        | InteractionAction::Uncertain
        | InteractionAction::Useful
        | InteractionAction::Credible
        | InteractionAction::NotCredible
        | InteractionAction::Recommend => InteractionVisibility::Public,
    }
}

pub fn allowed_actions(target: InteractionTargetType) -> &'static [InteractionAction] {
    match target {
        InteractionTargetType::Person => &[
            InteractionAction::Interest,
            InteractionAction::Follow,
            InteractionAction::Save,
        ],
        InteractionTargetType::Claim => &[
            InteractionAction::Support,
            InteractionAction::Dispute,
            InteractionAction::Uncertain,
            InteractionAction::Save,
        ],
        InteractionTargetType::Source => &[
            InteractionAction::Useful,
            InteractionAction::Credible,
            InteractionAction::NotCredible,
            InteractionAction::Save,
        ],
        InteractionTargetType::Place => &[
            InteractionAction::Save,
            InteractionAction::WantToVisit,
            InteractionAction::Visited,
        ],
        InteractionTargetType::Event => &[InteractionAction::Save],
        InteractionTargetType::Route => &[],
    }
}

pub fn is_allowed(action: InteractionAction, target: InteractionTargetType) -> bool {
    allowed_actions(target).contains(&action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_rejects_follow_claim_and_support_person() {
        assert!(!is_allowed(
            InteractionAction::Follow,
            InteractionTargetType::Claim
        ));
        assert!(!is_allowed(
            InteractionAction::Support,
            InteractionTargetType::Person
        ));
        assert!(is_allowed(
            InteractionAction::Save,
            InteractionTargetType::Event
        ));
        assert!(is_allowed(
            InteractionAction::Save,
            InteractionTargetType::Claim
        ));
        assert!(!is_allowed(
            InteractionAction::Recommend,
            InteractionTargetType::Route
        ));
        assert!(!InteractionTargetType::Route.is_supported());
    }

    #[test]
    fn json_snake_case() {
        let a = serde_json::to_string(&InteractionAction::NotCredible).unwrap();
        assert_eq!(a, "\"not_credible\"");
        let t: InteractionTargetType = serde_json::from_str("\"person\"").unwrap();
        assert_eq!(t, InteractionTargetType::Person);
    }
}
