require 'rails_helper'

RSpec.describe 'Talaria pipeline services' do
  it 'ingests fragments extracts judges and promotes deterministically' do
    doc = Talaria::Ingestion::IngestDocument.call(source_type: 'archive', source_uri: 'https://example.test/a', normalized_text: 'In 1901 an event happened in Rome.')
    Talaria::Fragmentation::FragmentDocument.call(doc)
    Talaria::Extraction::ExtractCandidates.call(doc)

    candidate = doc.raw_fragments.first.raw_candidates.first
    expect(candidate).to be_present

    Talaria::Extraction::RunAiJudgments.call(candidate)
    event = Talaria::Promotion::PromoteCandidateToEvent.call(candidate)

    expect(event).to be_persisted
    expect(event.event_evidence.count).to eq(1)
  end
end
