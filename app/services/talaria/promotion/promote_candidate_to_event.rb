module Talaria
  module Promotion
    class PromoteCandidateToEvent
      REQUIRED_CONFIDENCE = 0.6

      def self.call(candidate) = new(candidate).call
      def initialize(candidate) = @candidate = candidate

      def call
        payload = @candidate.structured_payload.with_indifferent_access
        raise 'Missing subject relevance' unless payload[:subject_relevant]
        raise 'Missing temporal validity' unless payload[:temporal_valid]

        judgment = @candidate.raw_ai_judgments.order(created_at: :desc).first
        raise 'Evidence required' unless judgment&.confidence.to_f >= REQUIRED_CONFIDENCE

        event = Event.create!(
          event_type: payload[:event_type] || 'historical',
          title: payload[:title] || @candidate.raw_fragment.text.first(80),
          summary: payload[:summary],
          confidence: @candidate.confidence,
          canonical_score: [@candidate.confidence, judgment.confidence].sum / 2.0,
          map_status: 'ready'
        )

        EventEvidence.create!(
          event: event,
          raw_document: @candidate.raw_fragment.raw_document,
          raw_fragment: @candidate.raw_fragment,
          raw_candidate: @candidate,
          raw_ai_judgment: judgment,
          quoted_text: @candidate.raw_fragment.text.first(500),
          evidence_type: 'text_fragment',
          source_kind: @candidate.raw_fragment.raw_document.source_type,
          confidence: judgment.confidence,
          review_status: 'approved'
        )

        @candidate.update!(status: :promoted)
        event
      end
    end
  end
end
