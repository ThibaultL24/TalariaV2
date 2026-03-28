module ConfidenceValidatable
  extend ActiveSupport::Concern

  included do
    validates :confidence, numericality: { greater_than_or_equal_to: 0, less_than_or_equal_to: 1 }
  end
end
