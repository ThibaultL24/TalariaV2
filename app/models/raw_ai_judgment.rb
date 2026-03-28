class RawAiJudgment < ApplicationRecord
  include ConfidenceValidatable

  belongs_to :raw_candidate, optional: true
  belongs_to :raw_fragment, optional: true

  enum :status, { pending: 'pending', approved: 'approved', rejected: 'rejected' }, validate: true
end
