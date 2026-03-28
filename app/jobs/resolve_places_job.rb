class ResolvePlacesJob < ApplicationJob
  queue_as :default
  def perform(raw_candidate_id)
    Talaria::Resolution::ResolvePlaces.call(RawCandidate.find(raw_candidate_id))
  end
end
