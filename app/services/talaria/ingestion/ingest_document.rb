module Talaria
  module Ingestion
    class IngestDocument
      def self.call(...) = new(...).call

      def initialize(source_type:, source_uri:, title: nil, language: nil, normalized_text: nil, payload: {})
        @source_type = source_type
        @source_uri = source_uri
        @title = title
        @language = language
        @normalized_text = normalized_text
        @payload = payload
      end

      def call
        run = IngestionRun.create!(status: :running, started_at: Time.current)
        doc = run.raw_documents.create!(
          source_type: @source_type,
          source_uri: @source_uri,
          title: @title,
          language: @language,
          normalized_text: @normalized_text,
          payload: @payload,
          fetched_at: Time.current,
          content_hash: Digest::SHA256.hexdigest(@normalized_text.to_s)
        )
        doc.source_snapshots.create!(storage_key: "snapshots/#{doc.id}", byte_size: @normalized_text.to_s.bytesize, sha256: doc.content_hash)
        run.update!(status: :completed, ended_at: Time.current)
        doc
      rescue StandardError
        run&.update(status: :failed, ended_at: Time.current)
        raise
      end
    end
  end
end
