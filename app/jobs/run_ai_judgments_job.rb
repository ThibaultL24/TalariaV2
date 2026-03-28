class RunAiJudgmentsJob < ApplicationJob
  queue_as :default
  def perform(raw_candidate_id)
    Talaria::Extraction::RunAiJudgments.call(RawCandidate.find(raw_candidate_id))
  end
end
