FactoryBot.define do
  factory :entity do
    entity_type { 'person' }
    canonical_name { Faker::Name.name }
    status { 'active' }
  end
end
