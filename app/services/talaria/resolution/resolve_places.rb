module Talaria
  module Resolution
    class ResolvePlaces
      class DeterministicResolver
        LOOKUP = {
          'rome' => [12.4964, 41.9028],
          'london' => [-0.1276, 51.5072]
        }.freeze

        def resolve(query)
          point = LOOKUP[query.to_s.downcase]
          return unless point

          { label: query.capitalize, lon: point[0], lat: point[1], precision: 'city', confidence: 0.75 }
        end
      end

      def self.call(candidate, resolver: DeterministicResolver.new) = new(candidate, resolver:).call
      def initialize(candidate, resolver:) = (@candidate, @resolver = candidate, resolver)

      def call
        query = @candidate.structured_payload['place'] || @candidate.structured_payload[:place]
        result = @resolver.resolve(query)
        return unless result

        RawPlaceResolution.create!(
          raw_candidate: @candidate,
          resolver_name: @resolver.class.name,
          query_text: query,
          resolved_label: result[:label],
          geom: "POINT(#{result[:lon]} #{result[:lat]})",
          geo_precision: result[:precision],
          confidence: result[:confidence],
          accepted: result[:confidence] >= 0.7
        )
      end
    end
  end
end
