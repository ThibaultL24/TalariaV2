module Talaria
  module Fragmentation
    class FragmentDocument
      def self.call(document) = new(document).call
      def initialize(document) = @document = document

      def call
        @document.normalized_text.to_s.split(/\n{2,}/).map.with_index do |chunk, idx|
          @document.raw_fragments.create!(text: chunk.strip, ordinal: idx + 1, fragment_kind: 'paragraph')
        end
      end
    end
  end
end
