class IngestDocumentJob < ApplicationJob
  queue_as :default
  def perform(params)
    Talaria::Ingestion::IngestDocument.call(**params.symbolize_keys)
  end
end
