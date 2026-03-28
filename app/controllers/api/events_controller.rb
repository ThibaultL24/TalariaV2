module Api
  class EventsController < ApplicationController
    def show
      render json: EventSerializer.new(Event.find(params[:id])).serializable_hash
    end

    def evidence
      render json: Event.find(params[:id]).event_evidence.order(confidence: :desc)
    end

    def export_graph
      render json: Talaria::GraphExport::ExportEventPayload.call(Event.find(params[:id]))
    end
  end
end
