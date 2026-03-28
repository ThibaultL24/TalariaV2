require 'rails_helper'

RSpec.configure do |config|
  config.openapi_root = Rails.root.join('openapi').to_s
  config.openapi_specs = {
    'v1/openapi.json' => {
      openapi: '3.0.1',
      info: { title: 'Talaria API', version: 'v1' },
      paths: {},
      servers: [{ url: 'http://localhost:3000' }]
    }
  }
  config.openapi_format = :json
end
