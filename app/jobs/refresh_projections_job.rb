class RefreshProjectionsJob < ApplicationJob
  queue_as :default
  def perform
    Talaria::Projection::GeojsonBuilder.call
    Talaria::Projection::TimelineBuilder.call
  end
end
