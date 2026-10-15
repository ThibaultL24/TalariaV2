// web/src/features/visit/visit-heritage-list.tsx
import type { TimelineEvent } from "@/lib/api";
import { eventDate } from "@/lib/entity-views";
import { eventTypeLabel } from "@/lib/event-taxonomy";
import { useI18n } from "@/lib/i18n";

export function VisitHeritageList({
  events,
  loading,
  onSelect,
  emptyLabel,
}: {
  events: TimelineEvent[];
  loading?: boolean;
  onSelect: (id: string) => void;
  emptyLabel: string;
}) {
  const { locale, t } = useI18n();
  if (loading && !events.length) {
    return <p className="v3-note">{t.loading}</p>;
  }
  if (!events.length) {
    return <p className="v3-note">{emptyLabel}</p>;
  }
  return (
    <ul className="visit-heritage-list">
      {events.map((event) => (
        <li key={event.id}>
          <button type="button" onClick={() => onSelect(event.id)}>
            <span className="visit-heritage-list__meta">
              {eventTypeLabel(event.event_type, locale)} · {eventDate(event)}
            </span>
            <span className="visit-heritage-list__title">{event.title}</span>
            {event.place_label ? (
              <span className="visit-heritage-list__place">{event.place_label}</span>
            ) : null}
          </button>
        </li>
      ))}
    </ul>
  );
}
