FactoryBot.define do
  factory :raw_document do
    source_type { 'archive' }
    source_uri { 'https://example.test/doc/1' }
    normalized_text { "In 1901 an event happened in Rome.\n\nAnother paragraph." }
  end
end
