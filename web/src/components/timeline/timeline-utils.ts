// web/src/components/timeline/timeline-utils.ts
import type { TimelineEvent } from "@/lib/api";
import { legendKeyForEventType } from "@/lib/event-legend";

export type YearWindow = [number, number];
export type TimeDomain = { start: number; end: number };


export function hashString(value: string): number {
  let h = 2166136261;
  for (let i = 0; i < value.length; i++) {
    h ^= value.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return Math.abs(h);
}

export function randomLaneFromId(id: string): number {
  return (hashString(id) % 10_000) / 10_000;
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

/** Numeric year axis (supports BCE as negative years). */
export function yearWindowToDomain([from, to]: YearWindow): TimeDomain {
  return { start: from, end: Math.max(from + 1, to) };
}

export function domainToYearWindow(domain: TimeDomain): YearWindow {
  return [Math.floor(domain.start), Math.ceil(domain.end)];
}

export function eventYear(event: TimelineEvent): number | null {
  const start = event.time?.start;
  if (!start || event.time?.kind === "unknown") {
    if (event.start_time) {
      const match = event.start_time.match(/^(-?\d{4})/);
      if (match) return Number(match[1]);
    }
    return null;
  }
  const match = start.match(/^(-?\d{1,})/);
  if (!match) return null;
  const year = Number(match[1]);
  return Number.isFinite(year) ? year : null;
}

const TYPE_IMPORTANCE: Record<string, number> = {
  birth: 0.92,
  death: 0.92,
  battle: 1,
  siege: 0.95,
  marriage: 0.75,
  office: 0.8,
  diplomatic: 0.78,
  publication: 0.7,
  education: 0.65,
};

export function eventImportance(event: TimelineEvent): number {
  const base = TYPE_IMPORTANCE[event.event_type] ?? 0.42;
  const boost = event.map_eligible ? 0.08 : 0;
  const conf = event.confidence ?? 0.8;
  return clamp(base + boost + (conf - 0.8) * 0.15, 0.15, 1);
}

export function eventColor(event: TimelineEvent): number {
  const key = legendKeyForEventType(event.event_type);
  const palette: Record<string, number> = {
    life: 0x7dd3fc,
    conflict: 0xf87171,
    travel: 0x34d399,
    office: 0xc4b5fd,
    work: 0xfb923c,
    anecdote: 0xe8b84a,
    legacy: 0x94a3b8,
  };
  return palette[key] ?? 0xffffff;
}

export function yearToX(year: number, domain: TimeDomain, width: number): number {
  const duration = domain.end - domain.start;
  if (duration <= 0) return 0;
  return ((year - domain.start) / duration) * width;
}

export function xToYear(x: number, domain: TimeDomain, width: number): number {
  const duration = domain.end - domain.start;
  return domain.start + (x / width) * duration;
}

export function getDetailLevel(domain: TimeDomain): number {
  const years = domain.end - domain.start;
  if (years > 5000) return 0;
  if (years > 1000) return 1;
  if (years > 200) return 2;
  if (years > 50) return 3;
  if (years > 10) return 4;
  if (years > 1) return 5;
  return 6;
}

const IMPORTANCE_THRESHOLD = [0.98, 0.9, 0.75, 0.55, 0.35, 0.15, 0];

export function filterEventsByLod(events: TimelineEvent[], domain: TimeDomain): TimelineEvent[] {
  const level = getDetailLevel(domain);
  const minImportance = IMPORTANCE_THRESHOLD[level] ?? 0;
  return events.filter((event) => eventImportance(event) >= minImportance);
}

export function clampWindow(window: YearWindow, bounds: YearWindow): YearWindow {
  const full = Math.max(1, bounds[1] - bounds[0]);
  const span = Math.min(full, Math.max(1, window[1] - window[0]));
  const start = Math.max(bounds[0], Math.min(bounds[1] - span, window[0]));
  return [start, start + span];
}

export function zoomWindow(
  window: YearWindow,
  factor: number,
  anchor: number,
  bounds: YearWindow,
): YearWindow {
  const pivot = window[0] + (window[1] - window[0]) * anchor;
  const span = Math.max(1, (window[1] - window[0]) * factor);
  return clampWindow([pivot - span * anchor, pivot + span * (1 - anchor)], bounds);
}

/** Vertical stacking for same-year events (DOM fallback / tests). */
export function layoutDots(
  events: TimelineEvent[],
  from: number,
  to: number,
  width: number,
): { event: TimelineEvent; x: number; row: number }[] {
  const rows: number[][] = [];
  return events.flatMap((event) => {
    const year = eventYear(event);
    if (year === null || year > to || year < from) return [];
    const x = 22 + ((year - from) / Math.max(1, to - from)) * Math.max(1, width - 44);
    let row = rows.findIndex((xs) => xs.every((other) => Math.abs(other - x) >= 24));
    if (row < 0) {
      row = rows.length;
      rows.push([]);
    }
    rows[row].push(x);
    return [{ event, x, row }];
  });
}

export function lifePhasePresets(bounds: YearWindow): { id: string; label: string; window: YearWindow }[] {
  const [start, end] = bounds;
  const span = Math.max(1, end - start);
  const third = span / 3;
  return [
    { id: "full", label: "Full life", window: bounds },
    { id: "early", label: "Early years", window: [start, start + third] },
    { id: "middle", label: "Middle years", window: [start + third, start + 2 * third] },
    { id: "late", label: "Later years", window: [start + 2 * third, end] },
  ];
}
