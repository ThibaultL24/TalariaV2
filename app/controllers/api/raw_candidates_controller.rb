module Api
  class RawCandidatesController < ApplicationController
    def promote
      event = Talaria::Promotion::PromoteCandidateToEvent.call(RawCandidate.find(params[:id]))
      render json: EventSerializer.new(event).serializable_hash, status: :created
    end
  end
end
