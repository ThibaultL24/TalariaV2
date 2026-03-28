class EntitySerializer
  include JSONAPI::Serializer
  attributes :entity_type, :canonical_name, :description, :external_ids, :status
end
