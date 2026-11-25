// web/src/features/istoria/life-reading.ts
import type { TimelineEvent } from "@/lib/api";

export type LifeSpeed = 0.5 | 1 | 2 | 4;

export interface LifeBand {
  id: string;
  label: string;
  from: number;
  to: number;
}

const DATED = /^(-?\d{1,6})(?:-(\d{2}))?(?:-(\d{2}))?/;

export function eventYear(event: TimelineEvent): number | null {
  const raw = event.time?.start ?? event.start_time ?? "";
  const match = DATED.exec(raw);
  if (!match) return null;
  const year = Number(match[1]);
  return Number.isFinite(year) ? year : null;
}

function sortValue(event: TimelineEvent): number {
  const raw = event.time?.start ?? event.start_time ?? "";
  const match = DATED.exec(raw);
  if (!match) return Number.POSITIVE_INFINITY;
  const year = Number(match[1]);
  const month = Number(match[2] ?? 0);
  const day = Number(match[3] ?? 0);
  return year * 10000 + month * 100 + day;
}

export function compareEvents(left: TimelineEvent, right: TimelineEvent): number {
  const delta = sortValue(left) - sortValue(right);
  if (delta !== 0) return delta;
  return left.title.localeCompare(right.title, "fr");
}

export function eventPoint(event: TimelineEvent): [number, number] | null {
  if (!event.map_eligible) return null;
  const lat = event.coordinates?.lat;
  const lon = event.coordinates?.lon;
  if (lat == null || lon == null || !Number.isFinite(lat) || !Number.isFinite(lon)) return null;
  if (Math.abs(lat) > 90 || Math.abs(lon) > 180) return null;
  return [lon, lat];
}

export function lifeBands(events: TimelineEvent[]): LifeBand[] {
  const years = events.map(eventYear).filter((year): year is number => year !== null);
  if (years.length === 0) return [];
  const start = Math.floor(Math.min(...years) / 10) * 10;
  const end = Math.floor(Math.max(...years) / 10) * 10;
  const bands: LifeBand[] = [];
  for (let year = start; year <= end; year += 10) {
    bands.push({ id: String(year), label: `${year}–${year + 9}`, from: year, to: year + 9 });
  }
  return bands;
}

export function eventsInBand(events: TimelineEvent[], band: LifeBand | null): TimelineEvent[] {
  return events.filter((event) => {
    const year = eventYear(event);
    if (year === null) return false;
    if (!band) return true;
    return year >= band.from && year <= band.to;
  });
}

/** Break the line wherever a fact has no coordinates. The segments are a time order, not a travelled route. */
export function successionSegments(events: TimelineEvent[]): [number, number][][] {
  const segments: [number, number][][] = [];
  let current: [number, number][] = [];
  for (const event of events) {
    const point = eventPoint(event);
    if (!point) {
      if (current.length > 1) segments.push(current);
      current = [];
      continue;
    }
    current.push(point);
  }
  if (current.length > 1) segments.push(current);
  return segments;
}

export function playbackDelay(speed: LifeSpeed, reducedMotion: boolean): number {
  if (reducedMotion) return 0;
  return Math.round(1600 / speed);
}
