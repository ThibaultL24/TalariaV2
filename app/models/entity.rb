class Entity < ApplicationRecord
  belongs_to :merged_into, class_name: 'Entity', optional: true
  has_many :entity_aliases, dependent: :destroy
  has_many :events, foreign_key: :subject_entity_id, dependent: :nullify

  enum :status, { active: 'active', deprecated: 'deprecated', merged: 'merged' }, validate: true
  validates :canonical_name, :entity_type, presence: true
end
