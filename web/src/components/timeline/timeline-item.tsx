// web/src/components/timeline/timeline-item.tsx
import type { TimelineItem as TimelineItemType } from "@/features/events/mappers/timeline";
import { epistemicBadgeClass } from "@/lib/event-taxonomy";
import { useI18n } from "@/lib/i18n";

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
  place,
  isVisibleOnMap,
  selected,
  onClick,
  onHover,
}: TimelineItemProps) {
  const { t } = useI18n();
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
            {isVisibleOnMap ? t.onMapBadge : t.offMapBadge}
          </span>
        ) : null}
      </div>

      <div className="mt-1 font-medium">{title}</div>
      {place ? (
        <div className="mt-1 text-[11px] text-(--color-text-muted)">{place}</div>
      ) : null}
      {confidence != null ? (
        <div className="mt-2 text-xs opacity-70">
          {t.modelConfidence(Math.round(confidence * 100))}
        </div>
      ) : null}
    </button>
  );
}
