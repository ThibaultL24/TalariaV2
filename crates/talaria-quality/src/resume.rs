// crates/talaria-quality/src/resume.rs
//! Idempotent retry policy for already-persisted event_candidates.

/// What a later ingest pass should do with a candidate that already exists
/// under the same fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExistingCandidateAction {
    /// Terminal: assembled or rejected. Skip — do not re-judge or reinforce.
    SkipTerminal,
    /// Crash window after insert, before a durable judgment. Re-run gates.
    ResumeFromGates,
    /// Gates already accepted; assemble may have crashed. Skip gates (avoids
    /// false singleton rejects if the event row already exists) and assemble.
    ResumeAssembleOnly,
}

/// Decide how to treat a fingerprint conflict on ingest retry.
///
/// `assembled` / `rejected` are terminal. `accepted` must resume assemble
/// without re-applying singleton gates. Anything else (`pending`,
/// `needs_review`, unknown) re-enters the gate path.
pub fn existing_candidate_action(status: &str) -> ExistingCandidateAction {
    match status {
        "assembled" | "rejected" => ExistingCandidateAction::SkipTerminal,
        "accepted" => ExistingCandidateAction::ResumeAssembleOnly,
        _ => ExistingCandidateAction::ResumeFromGates,
    }
}

/// A retry of the same candidate row must not increment source_count.
/// A distinct new candidate that matches an existing event fingerprint should.
pub fn should_reinforce_existing_event(is_new_candidate: bool) -> bool {
    is_new_candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembled_and_rejected_are_terminal() {
        assert_eq!(
            existing_candidate_action("assembled"),
            ExistingCandidateAction::SkipTerminal
        );
        assert_eq!(
            existing_candidate_action("rejected"),
            ExistingCandidateAction::SkipTerminal
        );
    }

    #[test]
    fn pending_retries_gates() {
        assert_eq!(
            existing_candidate_action("pending"),
            ExistingCandidateAction::ResumeFromGates
        );
        assert_eq!(
            existing_candidate_action("needs_review"),
            ExistingCandidateAction::ResumeFromGates
        );
        assert_eq!(
            existing_candidate_action("unknown-status"),
            ExistingCandidateAction::ResumeFromGates
        );
    }

    #[test]
    fn accepted_skips_gates_to_avoid_singleton_false_reject() {
        assert_eq!(
            existing_candidate_action("accepted"),
            ExistingCandidateAction::ResumeAssembleOnly
        );
    }

    #[test]
    fn same_row_retry_does_not_reinforce() {
        assert!(!should_reinforce_existing_event(false));
        assert!(should_reinforce_existing_event(true));
    }
}
