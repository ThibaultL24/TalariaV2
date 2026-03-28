class EntityLink < ApplicationRecord
  include ConfidenceValidatable
  belongs_to :from_entity, class_name: 'Entity'
  belongs_to :to_entity, class_name: 'Entity'
end
