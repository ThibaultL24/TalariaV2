class Claim < ApplicationRecord
  include ConfidenceValidatable
  belongs_to :subject_entity, class_name: 'Entity'
  belongs_to :object_entity, class_name: 'Entity', optional: true
  has_many :event_evidence, dependent: :nullify
end
