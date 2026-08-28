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

function timelineEventDateLabel(event: TimelineEvent): string {
  if (event.time?.surface) return event.time.surface;
  return formatDateLabel(event.time?.start ?? event.start_time);
}

export function mapTimelineEventToItem(event: TimelineEvent): TimelineItem {
  return {
    id: event.id,
    title: event.title,
    dateLabel: timelineEventDateLabel(event),
    eventType: eventTypeLabel(event.event_type),
    eventTypeKey: event.event_type,
    epistemicStatus: epistemicStatusLabel(event.epistemic_status),
    epistemicStatusKey: event.epistemic_status,
    place: event.place_label ?? undefined,
    year: timelineEventYear(event),
    isVisibleOnMap: event.map_eligible,
  };
}
