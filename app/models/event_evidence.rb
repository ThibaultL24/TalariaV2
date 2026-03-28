class EventEvidence < ApplicationRecord
  include ConfidenceValidatable

  belongs_to :event, optional: true
  belongs_to :claim, optional: true
  belongs_to :raw_document
  belongs_to :raw_fragment
  belongs_to :raw_candidate, optional: true
  belongs_to :raw_ai_judgment, optional: true

  enum :review_status, { pending: 'pending', approved: 'approved', rejected: 'rejected' }, validate: true
end
