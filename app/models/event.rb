class Event < ApplicationRecord
  include ConfidenceValidatable

  belongs_to :subject_entity, class_name: 'Entity', optional: true
  belongs_to :place, optional: true
  has_many :event_participants, dependent: :destroy
  has_many :event_evidence, dependent: :destroy

  validates :title, :event_type, presence: true
end
