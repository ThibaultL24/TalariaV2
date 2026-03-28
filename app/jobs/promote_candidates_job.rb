class PromoteCandidatesJob < ApplicationJob
  queue_as :default
  def perform(raw_candidate_id)
    Talaria::Promotion::PromoteCandidateToEvent.call(RawCandidate.find(raw_candidate_id))
  end
end
