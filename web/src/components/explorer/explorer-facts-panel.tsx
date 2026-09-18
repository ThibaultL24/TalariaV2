// web/src/components/explorer/explorer-facts-panel.tsx
import { mapTimelineEventToItem } from "@/features/events/mappers/timeline";
import type { TimelineEvent } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import { TimelineList } from "@/components/timeline/timeline-list";

interface ExplorerFactsPanelProps {
  events: TimelineEvent[];
  mapPinCount: number;
  isLoading: boolean;
  onSelectEvent: (id: string) => void;
}

export function ExplorerFactsPanel({
  events,
  mapPinCount,
  isLoading,
  onSelectEvent,
}: ExplorerFactsPanelProps) {
  const { t, locale } = useI18n();
  const items = events.map((event) => mapTimelineEventToItem(event, locale));
  const offMap = Math.max(0, events.length - mapPinCount);

  return (
    <aside
      className="surface-nav pointer-events-auto absolute z-20 flex flex-col overflow-hidden top-3 right-3 bottom-28 w-[min(100%-1.5rem,20rem)] max-md:inset-x-3 max-md:top-auto max-md:bottom-24 max-md:h-[38vh] max-md:w-auto"
      aria-label={t.factsTitle}
    >
      <header className="shrink-0 border-b border-(--color-border-subtle) px-3 py-2">
        <p className="text-[10px] font-semibold uppercase tracking-wide text-(--color-text-muted)">
          {t.factsTitle}
        </p>
        <p className="mt-0.5 text-[11px] tabular-nums text-(--color-text-secondary)">
          {t.factsSplit(mapPinCount, offMap, events.length)}
        </p>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto">
        <TimelineList
          items={items}
          hasEntity
          isLoading={isLoading}
          onSelectEvent={onSelectEvent}
        />
      </div>
    </aside>
  );
}
