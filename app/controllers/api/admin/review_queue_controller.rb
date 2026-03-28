module Api
  module Admin
    class ReviewQueueController < ApplicationController
      def index
        items = EventEvidence.where(review_status: 'pending').or(EventEvidence.where('confidence < 0.6')).order(created_at: :desc).limit(100)
        render json: { items: }
      end

      def update
        evidence = EventEvidence.find(params[:id])
        evidence.update!(review_status: params.require(:review_status))
        PromoteCandidatesJob.perform_later(evidence.raw_candidate_id) if evidence.review_status == 'approved' && evidence.raw_candidate_id
        render json: { id: evidence.id, review_status: evidence.review_status }
      end
    end
  end
end
