// web/src/components/timeline/timeline-item.tsx
import type { TimelineItem as TimelineItemType } from "@/features/events/mappers/timeline";
import { epistemicBadgeClass } from "@/lib/event-taxonomy";

interface TimelineItemProps extends TimelineItemType {
  selected?: boolean;
  onClick: () => void;
  onHover: (hovered: boolean) => void;
}

export function TimelineItem({
  title,
  dateLabel,
  eventType,
  epistemicStatus,
  epistemicStatusKey,
  confidence,
  isVisibleOnMap,
  selected,
  onClick,
  onHover,
}: TimelineItemProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      onMouseEnter={() => onHover(true)}
      onMouseLeave={() => onHover(false)}
      className={`nebula-timeline-card relative w-full p-3 text-left transition-colors ${
        selected ? "is-selected" : ""
      }`}
    >
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs uppercase text-(--color-text-secondary)">{eventType}</span>
        <span className="text-xs opacity-70">{dateLabel}</span>
      </div>

      <div className="mt-1.5 flex flex-wrap gap-1.5">
        <span
          className={`inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-medium ${epistemicBadgeClass(epistemicStatusKey)}`}
        >
          {epistemicStatus}
        </span>
        {typeof isVisibleOnMap === "boolean" ? (
          <span
            className={`inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-medium ${
              isVisibleOnMap
                ? "bg-emerald-500/10 text-emerald-200"
                : "bg-amber-500/12 text-amber-200"
            }`}
          >
            {isVisibleOnMap ? "On map" : "No coords"}
          </span>
        ) : null}
      </div>

      <div className="mt-1 font-medium">{title}</div>
      {confidence != null ? (
        <div className="mt-2 text-xs opacity-70">
          Model confidence: {Math.round(confidence * 100)}%
        </div>
      ) : null}
    </button>
  );
}
