module Talaria
  module Extraction
    class ExtractCandidates
      def self.call(document) = new(document).call
      def initialize(document) = @document = document

      def call
        @document.raw_fragments.find_each do |fragment|
          next unless fragment.text.match?(/\b(on|in)\s+\d{4}\b/i)

          fragment.raw_candidates.create!(
            candidate_kind: 'event',
            extractor_name: 'regex_extractor',
            extractor_version: '1.0.0',
            confidence: 0.7,
            structured_payload: { title: fragment.text.first(120), subject_relevant: true, temporal_valid: true }
          )
        end
      end
    end
  end
end
