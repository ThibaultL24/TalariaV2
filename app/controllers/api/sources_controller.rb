module Api
  class SourcesController < ApplicationController
    def import
      doc = Talaria::Ingestion::IngestDocument.call(source_type: params[:source_type], source_uri: params[:source_uri], normalized_text: params[:normalized_text])
      FragmentDocumentJob.perform_later(doc.id)
      render json: { raw_document_id: doc.id }, status: :accepted
    end
  end
end
