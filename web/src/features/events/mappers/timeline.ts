// web/src/features/events/mappers/timeline.ts
import { formatDateLabel } from "@/lib/geo";
import { eventTypeLabel, epistemicStatusLabel } from "@/lib/event-taxonomy";
import type { TimelineEvent } from "@/lib/api";

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

export function mapTimelineEventToItem(event: TimelineEvent): TimelineItem {
  const year = event.start_time ? Number.parseInt(event.start_time.slice(0, 4), 10) : null;
  return {
    id: event.id,
    title: event.title,
    dateLabel: formatDateLabel(event.start_time),
    eventType: eventTypeLabel(event.event_type),
    eventTypeKey: event.event_type,
    epistemicStatus: epistemicStatusLabel(event.epistemic_status),
    epistemicStatusKey: event.epistemic_status,
    confidence: event.confidence,
    place: event.place_label ?? undefined,
    year: Number.isFinite(year ?? NaN) ? year : null,
    isVisibleOnMap: event.map_eligible,
  };
}
