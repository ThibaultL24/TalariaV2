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

/** Year, or year plus month/day when the source date carries them. */
export function eventTimeValue(event: TimelineEvent): number | null {
  const raw =
    !event.time?.start || event.time.kind === "unknown"
      ? event.start_time
      : event.time.start;
  if (!raw) return null;
  const match = raw.match(/^(-?\d{1,6})(?:-(\d{2}))?(?:-(\d{2}))?/);
  if (!match) return null;
  const year = Number(match[1]);
  if (!Number.isFinite(year)) return null;
  const month = match[2] ? Number(match[2]) : 0;
  const day = match[3] ? Number(match[3]) : 0;
  if (!month) return year;
  const fraction = (Math.min(12, Math.max(1, month)) - 1) / 12;
  const dayFraction = day ? (Math.min(28, Math.max(1, day)) - 1) / 365 : 0;
  return year + fraction + dayFraction;
}

export function eventYear(event: TimelineEvent): number | null {
  const raw =
    !event.time?.start || event.time.kind === "unknown"
      ? event.start_time
      : event.time.start;
  if (!raw) return null;
  const match = raw.match(/^(-?\d{1,6})/);
  if (!match) return null;
  const year = Number(match[1]);
  return Number.isFinite(year) ? year : null;
}

export function formatYearLabel(value: number, locale: "fr" | "en" = "en"): string {
  const year = Math.floor(value + 1e-8);
  const fraction = value - year;
  const month = fraction > 0.04 && fraction < 0.98
    ? Math.min(12, Math.floor(fraction * 12) + 1)
    : 0;
  if (year < 0) {
    const abs = Math.abs(year);
    const body = month ? `${abs}-${String(month).padStart(2, "0")}` : String(abs);
    return locale === "fr" ? `${body} av. n. è.` : `${body} BCE`;
  }
  if (month) return `${year}-${String(month).padStart(2, "0")}`;
  return String(year);
}

export function axisTicks(domain: TimeDomain): number[] {
  const span = Math.max(0, domain.end - domain.start);
  const step =
    span > 2000 ? 500
    : span > 400 ? 100
    : span > 80 ? 10
    : span > 16 ? 5
    : span > 4 ? 1
    : span > 1.2 ? 0.25
    : 1 / 12;
  const ticks: number[] = [];
  const first = Math.ceil((domain.start - 1e-6) / step) * step;
  for (let value = first; value <= domain.end + 1e-6 && ticks.length < 14; value += step) {
    ticks.push(Math.round(value * 1000) / 1000);
  }
  return ticks;
}

const TYPE_IMPORTANCE: Record<string, number> = {
  birth: 1,
  death: 1,
  battle: 0.98,
  siege: 0.9,
  marriage: 0.74,
  office: 0.76,
  diplomatic: 0.72,
  publication: 0.64,
  education: 0.58,
};

export type VisualRank = "major" | "secondary" | "minor";

/** Historical weight plus documentary confidence. A map pin does not raise it. */
export function eventImportance(event: TimelineEvent): number {
  const base = TYPE_IMPORTANCE[event.event_type] ?? 0.4;
  const confidence = event.confidence ?? 0.8;
  return clamp(base + (confidence - 0.8) * 0.12, 0.12, 1);
}

export function eventRank(importance: number): VisualRank {
  if (importance >= 0.995) return "major";
  if (importance >= 0.62) return "secondary";
  return "minor";
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
  const years = domain.end - domain.start;
  if (events.length <= 1200 && years <= 250) return events;
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

export function separateStackedPoints<T extends { x: number; y: number }>(points: T[], minGap = 18): T[] {
  const placed: T[] = [];
  for (const point of points) {
    let y = point.y;
    let guard = 0;
    while (
      placed.some((other) => Math.abs(other.x - point.x) < 14 && Math.abs(other.y - y) < minGap)
      && guard < 8
    ) {
      y += minGap;
      guard += 1;
    }
    placed.push({ ...point, y });
  }
  return placed;
}
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

export interface DocumentedLifePhase {
  id: string;
  label: string;
  window: YearWindow;
}

export interface LifePhasePreset {
  id: string;
  label: string;
  window: YearWindow;
  /** True when the window comes from a documented biographical period. */
  documented: boolean;
}

export function lifePhasePresets(
  bounds: YearWindow,
  documented: DocumentedLifePhase[] = [],
): LifePhasePreset[] {
  const known = documented.filter((phase) => phase.window[1] > phase.window[0]);
  if (known.length > 0) {
    return [
      { id: "full", label: "Full span", window: bounds, documented: false },
      ...known.map((phase) => ({ ...phase, documented: true })),
    ];
  }
  const [start, end] = bounds;
  const span = Math.max(1, end - start);
  const third = span / 3;
  return [
    { id: "full", label: "Full span", window: bounds, documented: false },
    { id: "opening", label: "Opening third", window: [start, start + third], documented: false },
    { id: "middle", label: "Middle third", window: [start + third, start + 2 * third], documented: false },
    { id: "closing", label: "Closing third", window: [start + 2 * third, end], documented: false },
  ];
}

const FAMILY_ANCHOR: Record<string, number> = {
  life: 0.18,
  conflict: 0.33,
  office: 0.46,
  work: 0.57,
  travel: 0.7,
  anecdote: 0.8,
  legacy: 0.9,
};

export interface ConstellationPoint {
  event: TimelineEvent;
  time: number;
  x: number;
  y: number;
  rank: VisualRank;
  importance: number;
  color: number;
  radius: number;
  alpha: number;
}

function unitFromId(id: string, salt: number): number {
  return (hashString(`${salt}:${id}`) % 10_000) / 10_000;
}

/** Stable family constellations. Dates stay on the time axis; Y is not a random lane. */
export function layoutConstellation(
  events: TimelineEvent[],
  domain: TimeDomain,
  width: number,
  height: number,
  padding: { top: number; bottom: number } = { top: 44, bottom: 30 },
): ConstellationPoint[] {
  const usable = Math.max(80, height - padding.top - padding.bottom);
  const minY = padding.top + 6;
  const maxY = padding.top + usable - 6;
  const rough: ConstellationPoint[] = [];
  for (const event of events) {
    const time = eventTimeValue(event);
    if (time === null || time < domain.start || time > domain.end) continue;
    const family = legendKeyForEventType(event.event_type);
    const anchor = FAMILY_ANCHOR[family] ?? 0.55;
    const sway = (unitFromId(event.id, 1) - 0.5) * 0.34;
    const lane = clamp(anchor + sway, 0.04, 0.96);
    const yearPrecision = event.time?.precision !== "month" && event.time?.precision !== "day";
    const jitter = yearPrecision ? (unitFromId(event.id, 4) - 0.5) * 0.65 : 0;
    const plotTime = time + jitter;
    const importance = eventImportance(event);
    const rank = eventRank(importance);
    rough.push({
      event,
      time: plotTime,
      x: yearToX(plotTime, domain, width),
      y: padding.top + lane * usable,
      rank,
      importance,
      color: eventColor(event),
      radius: rank === "major" ? 7.2 : rank === "secondary" ? 3.6 : 2.7,
      alpha: 1,
    });
  }
  separateCloud(rough, minY, maxY);
  return rough;
}

/** Nudge overlapping dots just enough to stay separate, without stacking a whole year onto one line. */
function separateCloud(points: ConstellationPoint[], minY: number, maxY: number) {
  const cell = 14;
  const grid = new Map<number, ConstellationPoint[]>();
  const ordered = [...points].sort(
    (a, b) => a.time - b.time || b.importance - a.importance || a.event.id.localeCompare(b.event.id),
  );
  for (const point of ordered) {
    const minDistance = point.radius * 2 + 2;
    let y = point.y;
    for (let attempt = 0; attempt < 14; attempt += 1) {
      if (!cloudHits(grid, point.x, y, minDistance, cell)) break;
      const direction = attempt % 2 === 0 ? 1 : -1;
      const step = Math.ceil(attempt / 2) + 1;
      y = clamp(point.y + direction * step * minDistance * 0.55, minY, maxY);
    }
    point.y = y;
    const key = Math.floor(point.x / cell);
    const column = grid.get(key) ?? [];
    column.push(point);
    grid.set(key, column);
  }
}

function cloudHits(
  grid: Map<number, ConstellationPoint[]>,
  x: number,
  y: number,
  minDistance: number,
  cell: number,
): boolean {
  const column = Math.floor(x / cell);
  for (let index = column - 2; index <= column + 2; index += 1) {
    const bucket = grid.get(index);
    if (!bucket) continue;
    for (const other of bucket) {
      if (Math.abs(other.x - x) < minDistance && Math.abs(other.y - y) < minDistance) return true;
    }
  }
  return false;
}

export function densityMarks(
  events: TimelineEvent[],
  domain: TimeDomain,
  width: number,
  columns = 36,
): { x: number; count: number }[] {
  const bins = Array<number>(columns).fill(0);
  const span = Math.max(1e-6, domain.end - domain.start);
  for (const event of events) {
    const time = eventTimeValue(event);
    if (time === null || time < domain.start || time > domain.end) continue;
    const index = Math.min(columns - 1, Math.max(0, Math.floor(((time - domain.start) / span) * columns)));
    bins[index] += 1;
  }
  return bins
    .map((count, index) => ({ x: ((index + 0.5) / columns) * width, count }))
    .filter((bin) => bin.count > 0);
}
