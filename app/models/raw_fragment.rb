class RawFragment < ApplicationRecord
  belongs_to :raw_document
  has_many :raw_candidates, dependent: :destroy
  has_many :raw_ai_judgments, dependent: :destroy

  validates :text, :ordinal, presence: true
end
