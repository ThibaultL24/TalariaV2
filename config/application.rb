require_relative 'boot'
require 'rails/all'
Bundler.require(*Rails.groups)

module Talaria
  class Application < Rails::Application
    config.load_defaults 8.0
    config.api_only = true
    config.active_job.queue_adapter = :solid_queue
    config.eager_load_paths << Rails.root.join('app/services')
  end
end
