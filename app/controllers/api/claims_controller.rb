module Api
  class ClaimsController < ApplicationController
    def export_graph
      render json: Talaria::GraphExport::ExportEventPayload.call(Claim.find(params[:id]))
    end
  end
end
