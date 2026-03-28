require 'rails_helper'

RSpec.describe 'API', type: :request do
  it 'returns entity and related endpoints' do
    entity = create(:entity)
    get "/api/entities/#{entity.id}"
    expect(response).to have_http_status(:ok)
  end

  it 'returns map feature collection' do
    get '/api/map/features'
    expect(response).to have_http_status(:ok)
    expect(JSON.parse(response.body)['type']).to eq('FeatureCollection')
  end
end
