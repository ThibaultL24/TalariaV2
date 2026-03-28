class EventSerializer
  include JSONAPI::Serializer
  attributes :event_type, :title, :summary, :start_time, :end_time, :confidence, :canonical_score
end
