module Api
  class IngestionRunsController < ApplicationController
    def create
      doc = Talaria::Ingestion::IngestDocument.call(**ingestion_params.to_h.symbolize_keys)
      render json: { raw_document_id: doc.id }, status: :created
    end

    private

    def ingestion_params
      params.require(:ingestion).permit(:source_type, :source_uri, :title, :language, :normalized_text, payload: {})
    end
  end
end
