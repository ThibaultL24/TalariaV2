module Api
  class EntitiesController < ApplicationController
    def show
      render json: EntitySerializer.new(Entity.find(params[:id])).serializable_hash
    end

    def events
      render json: EventSerializer.new(Entity.find(params[:id]).events).serializable_hash
    end

    def graph_context
      entity = Entity.find(params[:id])
      render json: { entity: entity.attributes, links: EntityLink.where(from_entity: entity).limit(100) }
    end
  end
end
