module Talaria
  module Projection
    class GeojsonBuilder
      def self.call(events = Event.includes(:event_evidence))
        {
          type: 'FeatureCollection',
          features: events.map do |event|
            {
              type: 'Feature',
              id: event.id,
              geometry: nil,
              properties: { title: event.title, event_type: event.event_type, confidence: event.confidence }
            }
          end
        }
      end
    end
  end
end
