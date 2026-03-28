class FragmentDocumentJob < ApplicationJob
  queue_as :default
  def perform(raw_document_id)
    Talaria::Fragmentation::FragmentDocument.call(RawDocument.find(raw_document_id))
  end
end
