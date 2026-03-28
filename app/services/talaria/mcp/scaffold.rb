module Talaria
  module Mcp
    class Scaffold
      def self.resources
        [{ name: 'events', mode: 'read_only' }, { name: 'entities', mode: 'read_only' }]
      end
    end
  end
end
