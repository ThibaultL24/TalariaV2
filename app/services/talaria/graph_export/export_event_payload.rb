module Talaria
  module GraphExport
    class ExportEventPayload
      def self.call(record)
        {
          id: record.id,
          kind: record.class.name.underscore,
          attributes: record.attributes,
          edges: record.respond_to?(:event_evidence) ? record.event_evidence.pluck(:id) : []
        }
      end
    end
  end
end
