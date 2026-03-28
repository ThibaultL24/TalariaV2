module Talaria
  module Projection
    class TimelineBuilder
      def self.call(scope = Event.all)
        scope.order(:start_time).map do |event|
          { id: event.id, title: event.title, start_time: event.start_time, end_time: event.end_time, precision: event.date_precision }
        end
      end
    end
  end
end
