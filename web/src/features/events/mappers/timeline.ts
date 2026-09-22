// web/src/features/events/mappers/timeline.ts
import { localizedEventTitle, localizedPlaceLabel, localizedDateSurface } from "@/lib/localize-event-copy";
import { formatDateLabel } from "@/lib/geo";
import { eventTypeLabel, epistemicStatusLabel } from "@/lib/event-taxonomy";
import type { TimelineEvent } from "@/lib/api";
import type { AppLocale } from "@/lib/i18n";

export interface TimelineItem {
  id: string;
  title: string;
  dateLabel: string;
  eventType: string;
  eventTypeKey: string;
  epistemicStatus: string;
  epistemicStatusKey: string;
  confidence?: number;
  place?: string;
  year: number | null;
  isVisibleOnMap?: boolean;
}

function timelineEventYear(event: TimelineEvent): number | null {
  const fromTimeStart = event.time?.start;
  if (fromTimeStart) {
    const year = Number.parseInt(fromTimeStart.slice(0, 4), 10);
    if (Number.isFinite(year)) return year;
  }
  if (!event.start_time) return null;
  const year = Number.parseInt(event.start_time.slice(0, 4), 10);
  return Number.isFinite(year) ? year : null;
}

function timelineEventDateLabel(event: TimelineEvent, locale: AppLocale): string {
  if (event.time?.surface) return localizedDateSurface(event.time.surface, locale);
  return formatDateLabel(event.time?.start ?? event.start_time);
}

export function mapTimelineEventToItem(
  event: TimelineEvent,
  locale: AppLocale = "en",
): TimelineItem {
  return {
    id: event.id,
    title: localizedEventTitle(event, locale),
    dateLabel: timelineEventDateLabel(event, locale),
    eventType: eventTypeLabel(event.event_type, locale),
    eventTypeKey: event.event_type,
    epistemicStatus: epistemicStatusLabel(event.epistemic_status, locale),
    epistemicStatusKey: event.epistemic_status,
    place: localizedPlaceLabel(event.place_label, locale),
    year: timelineEventYear(event),
    isVisibleOnMap: event.map_eligible,
  };
}
