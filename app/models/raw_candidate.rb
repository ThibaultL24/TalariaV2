class RawCandidate < ApplicationRecord
  include ConfidenceValidatable

  belongs_to :raw_fragment
  has_many :raw_ai_judgments, dependent: :destroy
  has_many :raw_place_resolutions, dependent: :destroy

  enum :status, { pending: 'pending', accepted: 'accepted', rejected: 'rejected', promoted: 'promoted' }, validate: true
end
