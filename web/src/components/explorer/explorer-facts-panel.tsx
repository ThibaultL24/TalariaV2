// web/src/components/explorer/explorer-facts-panel.tsx
import { useMemo, useState } from "react";
import { TimelineList } from "@/components/timeline/timeline-list";
import { mapTimelineEventToItem } from "@/features/events/mappers/timeline";
import type { TimelineEvent } from "@/lib/api";
import { useI18n } from "@/lib/i18n";

interface ExplorerFactsPanelProps {
  events: TimelineEvent[];
  mapPinCount: number;
  isLoading: boolean;
  onSelectEvent: (id: string) => void;
}

type FactsTab = "all" | "onMap" | "offMap";

export function ExplorerFactsPanel({
  events,
  mapPinCount,
  isLoading,
  onSelectEvent,
}: ExplorerFactsPanelProps) {
  const { t, locale } = useI18n();
  const [tab, setTab] = useState<FactsTab>("all");

  const offMapCount = Math.max(0, events.length - mapPinCount);

  const filtered = useMemo(() => {
    if (tab === "onMap") return events.filter((event) => event.map_eligible);
    if (tab === "offMap") return events.filter((event) => !event.map_eligible);
    return events;
  }, [events, tab]);

  const items = filtered.map((event) => mapTimelineEventToItem(event, locale));

  const tabs: Array<{ id: FactsTab; label: string; count: number }> = [
    { id: "all", label: t.factsTabAll, count: events.length },
    { id: "onMap", label: t.factsTabOnMap, count: mapPinCount },
    { id: "offMap", label: t.factsTabOffMap, count: offMapCount },
  ];

  return (
    <aside
      className="explorer-facts surface-nav flex h-full min-h-0 flex-col overflow-hidden"
      aria-label={t.factsTitle}
    >
      <header className="shrink-0 border-b border-(--color-border-subtle) px-3 py-2">
        <p className="text-[10px] font-semibold uppercase tracking-wide text-(--color-text-muted)">
          {t.factsTitle}
        </p>
        <p className="mt-0.5 text-[11px] tabular-nums text-(--color-text-secondary)">
          {t.factsSplit(mapPinCount, offMapCount, events.length)}
        </p>
        <div
          className="mt-2 flex flex-wrap gap-1"
          role="tablist"
          aria-label={t.factsTitle}
        >
          {tabs.map((entry) => {
            const selected = tab === entry.id;
            return (
              <button
                key={entry.id}
                type="button"
                role="tab"
                aria-selected={selected}
                onClick={() => setTab(entry.id)}
                className={`rounded-md px-2 py-1 text-[10px] font-medium transition-colors ${
                  selected
                    ? "bg-white/12 text-(--color-text-primary) ring-1 ring-white/20"
                    : "text-(--color-text-muted) hover:bg-white/5 hover:text-(--color-text-secondary)"
                }`}
              >
                {entry.label}
                <span className="ml-1 tabular-nums opacity-65">{entry.count}</span>
              </button>
            );
          })}
        </div>
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
