class RawPlaceResolution < ApplicationRecord
  include ConfidenceValidatable

  belongs_to :raw_candidate
end
