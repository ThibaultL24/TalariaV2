class RawDocument < ApplicationRecord
  belongs_to :ingestion_run, optional: true
  has_many :raw_fragments, dependent: :destroy
  has_many :source_snapshots, dependent: :destroy
  has_many :event_evidence, dependent: :nullify

  validates :source_type, :source_uri, presence: true
end
