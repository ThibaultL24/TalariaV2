class ExtractCandidatesJob < ApplicationJob
  queue_as :default
  def perform(raw_document_id)
    Talaria::Extraction::ExtractCandidates.call(RawDocument.find(raw_document_id))
  end
end
