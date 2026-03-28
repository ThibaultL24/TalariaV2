module Api
  class RawDocumentsController < ApplicationController
    def reprocess
      document = RawDocument.find(params[:id])
      FragmentDocumentJob.perform_later(document.id)
      ExtractCandidatesJob.perform_later(document.id)
      head :accepted
    end
  end
end
