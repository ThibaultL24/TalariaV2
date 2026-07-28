// web/src/components/timeline/timeline-list.tsx
import { useExplorerStore } from "@/stores/explorer-store";
import type { TimelineItem as TimelineItemType } from "@/features/events/mappers/timeline";
import { TimelineItem } from "./timeline-item";

interface TimelineListProps {
  items: TimelineItemType[];
  hasEntity: boolean;
  isLoading?: boolean;
  onSelectEvent?: (id: string) => void;
}

export function TimelineList({
  items,
  hasEntity,
  isLoading,
  onSelectEvent,
}: TimelineListProps) {
  const { selectedEventId, setSelectedEventId, setHoveredEventId } = useExplorerStore();

  function handleSelect(id: string) {
    setSelectedEventId(id);
    onSelectEvent?.(id);
  }

  if (!hasEntity) {
    return (
      <div className="p-4 text-center text-sm leading-relaxed text-(--color-text-muted)">
        Search for a person to load events and map points.
      </div>
    );
  }

  if (isLoading && items.length === 0) {
    return <p className="p-4 text-center text-sm text-(--color-text-muted)">Loading timeline…</p>;
  }

  if (items.length === 0) {
    return (
      <div className="p-4 text-center text-sm text-(--color-text-muted)">
        No events for this period. Adjust filters or run the pipeline.
      </div>
    );
  }

  return (
    <div className="space-y-2 overflow-y-auto p-3">
      {items.map((item) => (
        <TimelineItem
          key={item.id}
          {...item}
          selected={selectedEventId === item.id}
          onClick={() => handleSelect(item.id)}
          onHover={(hovered) => setHoveredEventId(hovered ? item.id : undefined)}
        />
      ))}
    </div>
  );
}
