module Api
  class TimelineController < ApplicationController
    def index
      render json: { items: Talaria::Projection::TimelineBuilder.call(Event.all) }
    end
  end
end
