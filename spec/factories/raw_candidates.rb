FactoryBot.define do
  factory :raw_candidate do
    raw_fragment
    candidate_kind { 'event' }
    structured_payload { { title: 'Test Event', subject_relevant: true, temporal_valid: true, place: 'rome' } }
    extractor_name { 'spec' }
    extractor_version { '1.0' }
    confidence { 0.8 }
    status { 'pending' }
  end
end
