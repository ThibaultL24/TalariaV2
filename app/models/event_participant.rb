class EventParticipant < ApplicationRecord
  include ConfidenceValidatable
  belongs_to :event
  belongs_to :entity
end
