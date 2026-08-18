// web/src/lib/geo.ts
import type { GeoJsonFeatureCollection, TimelineEvent } from "./api";

export function extractYear(startTime?: string | null): number | null {
  if (!startTime) return null;
  const match = startTime.match(/^(-?\d+)/);
  if (!match) return null;
  const year = Number.parseInt(match[1], 10);
  return Number.isFinite(year) ? year : null;
}

export function ensureFeatureIds(collection: GeoJsonFeatureCollection): GeoJsonFeatureCollection {
  return {
    type: "FeatureCollection",
    features: collection.features.map((feature) => {
      const id =
        feature.id ??
        (typeof feature.properties.id === "string" ? feature.properties.id : undefined);
      return id ? { ...feature, id } : feature;
    }),
  };
}

export function filterGeoJsonByYearRange(
  collection: GeoJsonFeatureCollection,
  range: { min: number; max: number },
): GeoJsonFeatureCollection {
  return {
    type: "FeatureCollection",
    features: collection.features.filter((feature) => {
      const year = extractYear(String(feature.properties.start_time ?? ""));
      if (year == null) return true;
      return year >= range.min && year <= range.max;
    }),
  };
}

export function filterTimelineByYearRange(
  events: TimelineEvent[],
  range: { min: number; max: number },
): TimelineEvent[] {
  return events.filter((event) => {
    const year = extractYear(event.start_time);
    if (year == null) return true;
    return year >= range.min && year <= range.max;
  });
}

export function buildYearBounds(events: TimelineEvent[]): { min: number; max: number } {
  const years = events
    .map((event) => extractYear(event.start_time))
    .filter((year): year is number => year != null);
  if (years.length === 0) return { min: 1800, max: new Date().getFullYear() };
  return { min: Math.min(...years), max: Math.max(...years) };
}

export function buildYearHistogram(events: TimelineEvent[]): { year: number; count: number }[] {
  const counts = new Map<number, number>();
  for (const event of events) {
    const year = extractYear(event.start_time);
    if (year == null) continue;
    counts.set(year, (counts.get(year) ?? 0) + 1);
  }
  return [...counts.entries()]
    .map(([year, count]) => ({ year, count }))
    .sort((a, b) => a.year - b.year);
}

export function formatDateLabel(startTime?: string | null): string {
  const year = extractYear(startTime);
  return year != null ? String(year) : "—";
}

export function eventTypeLabel(eventType: string): string {
  return eventType.replace(/_/g, " ");
}

export function filterTimelineByTaxonomy(
  events: TimelineEvent[],
  types: string[],
  statuses: string[],
): TimelineEvent[] {
  return events.filter((event) => {
    if (types.length > 0 && !types.includes(event.event_type)) return false;
    if (statuses.length > 0 && !statuses.includes(event.epistemic_status)) return false;
    return true;
  });
}

export function filterGeoJsonByTaxonomy(
  collection: GeoJsonFeatureCollection,
  types: string[],
  statuses: string[],
): GeoJsonFeatureCollection {
  return {
    type: "FeatureCollection",
    features: collection.features.filter((feature) => {
      const eventType = String(feature.properties.event_type ?? "");
      const status = String(feature.properties.epistemic_status ?? "");
      if (types.length > 0 && !types.includes(eventType)) return false;
      if (statuses.length > 0 && !statuses.includes(status)) return false;
      return true;
    }),
  };
}
