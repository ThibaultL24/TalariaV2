FactoryBot.define do
  factory :raw_fragment do
    raw_document
    fragment_kind { 'paragraph' }
    ordinal { 1 }
    text { 'In 1901 an event happened in Rome.' }
  end
end
