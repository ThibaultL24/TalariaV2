class IngestionRun < ApplicationRecord
  has_many :raw_documents, dependent: :nullify
  enum :status, { pending: 'pending', running: 'running', completed: 'completed', failed: 'failed' }, validate: true
end
