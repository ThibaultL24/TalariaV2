module Api
  class MapController < ApplicationController
    def features
      render json: Talaria::Projection::GeojsonBuilder.call(Event.all)
    end
  end
end
