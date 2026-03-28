require 'rails_helper'

RSpec.describe 'E2E document -> geojson', type: :request do
  it 'runs full path into projection' do
    doc = Talaria::Ingestion::IngestDocument.call(source_type: 'archive', source_uri: 'https://example.test/x', normalized_text: 'In 1901 an event happened in Rome.')
    Talaria::Fragmentation::FragmentDocument.call(doc)
    Talaria::Extraction::ExtractCandidates.call(doc)
    candidate = doc.raw_fragments.first.raw_candidates.first
    Talaria::Extraction::RunAiJudgments.call(candidate)
    Talaria::Promotion::PromoteCandidateToEvent.call(candidate)

    get '/api/map/features'
    expect(response).to have_http_status(:ok)
    expect(JSON.parse(response.body)['features']).not_to be_empty
  end
end
