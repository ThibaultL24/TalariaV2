class Place < ApplicationRecord
  include ConfidenceValidatable
  has_many :events, dependent: :nullify
end
