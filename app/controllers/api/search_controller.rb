module Api
  class SearchController < ApplicationController
    def index
      q = params[:q].to_s
      events = Event.where('title ILIKE ?', "%#{q}%").limit(20)
      entities = Entity.where('canonical_name ILIKE ?', "%#{q}%").limit(20)
      render json: { events:, entities: }
    end
  end
end
