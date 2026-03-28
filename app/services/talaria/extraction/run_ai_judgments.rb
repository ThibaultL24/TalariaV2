module Talaria
  module Extraction
    class RunAiJudgments
      class Adapter
        def judge(candidate)
          { confidence: 0.65, result: { advisory: 'likely_valid', candidate_id: candidate.id } }
        end
      end

      def self.call(candidate, adapter: Adapter.new) = new(candidate, adapter:).call
      def initialize(candidate, adapter:) = (@candidate, @adapter = candidate, adapter)

      def call
        verdict = @adapter.judge(@candidate)
        RawAiJudgment.create!(
          raw_candidate: @candidate,
          raw_fragment: @candidate.raw_fragment,
          model_name: @adapter.class.name,
          task_name: 'candidate_review',
          result_json: verdict[:result],
          confidence: verdict[:confidence],
          payload_hash: Digest::SHA256.hexdigest(verdict[:result].to_json),
          status: 'approved'
        )
      end
    end
  end
end
