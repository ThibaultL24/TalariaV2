// web/src/lib/geo.ts
import type { GeoJsonFeatureCollection, TimelineEvent } from "./api";

function featurePoint(feature: unknown): [number, number] | null {
  if (!feature || typeof feature !== "object") return null;
  const geometry = (feature as { geometry?: { coordinates?: unknown } }).geometry;
  const coords = geometry?.coordinates;
  if (!Array.isArray(coords) || coords.length < 2) return null;
  const lon = Number(coords[0]);
  const lat = Number(coords[1]);
  if (!Number.isFinite(lon) || !Number.isFinite(lat)) return null;
  return [lon, lat];
}

export function boundsOfMapFeatures(collection: {
  features: unknown[];
}): [[number, number], [number, number]] | null {
  let minLon = Infinity;
  let minLat = Infinity;
  let maxLon = -Infinity;
  let maxLat = -Infinity;
  for (const feature of collection.features) {
    const point = featurePoint(feature);
    if (!point) continue;
    const [lon, lat] = point;
    minLon = Math.min(minLon, lon);
    minLat = Math.min(minLat, lat);
    maxLon = Math.max(maxLon, lon);
    maxLat = Math.max(maxLat, lat);
  }
  if (!Number.isFinite(minLon)) return null;
  if (minLon === maxLon) {
    minLon -= 0.5;
    maxLon += 0.5;
  }
  if (minLat === maxLat) {
    minLat -= 0.35;
    maxLat += 0.35;
  }
  return [
    [minLon, minLat],
    [maxLon, maxLat],
  ];
}

export function extractYear(startTime?: string | null): number | null {
  if (!startTime) return null;
  const match = startTime.match(/^(-?\d+)/);
  if (!match) return null;
  const year = Number.parseInt(match[1], 10);
  return Number.isFinite(year) ? year : null;
}

export function eventYear(event: TimelineEvent): number | null {
  return extractYear(event.time?.start) ?? extractYear(event.start_time);
}

const STACK_KEY_DECIMALS = 3;
const RING_STEP_DEG = 0.018;
const RING_SIZE = 8;

function coordKey(lon: number, lat: number): string {
  return `${lon.toFixed(STACK_KEY_DECIMALS)}|${lat.toFixed(STACK_KEY_DECIMALS)}`;
}

/** Same cell (city/country centroid) → ring so stacked events stay clickable. */
export function spreadStackedMapPoints(
  collection: GeoJsonFeatureCollection,
): GeoJsonFeatureCollection {
  const buckets = new Map<string, number[]>();
  collection.features.forEach((feature, index) => {
    const coords = feature.geometry?.coordinates;
    if (!coords || coords.length < 2) return;
    const key = coordKey(Number(coords[0]), Number(coords[1]));
    const list = buckets.get(key) ?? [];
    list.push(index);
    buckets.set(key, list);
  });

  return {
    type: "FeatureCollection",
    features: collection.features.map((feature, index) => {
      const coords = feature.geometry?.coordinates;
      if (!coords || coords.length < 2) return feature;
      const lon = Number(coords[0]);
      const lat = Number(coords[1]);
      const group = buckets.get(coordKey(lon, lat));
      if (!group || group.length < 2) return feature;
      const i = group.indexOf(index);
      const ring = Math.floor(i / RING_SIZE);
      const slot = i % RING_SIZE;
      const onRing = Math.min(RING_SIZE, group.length - ring * RING_SIZE);
      const angle = (Math.PI * 2 * slot) / onRing;
      const radius = RING_STEP_DEG * (ring + 1);
      return {
        ...feature,
        geometry: {
          ...feature.geometry,
          coordinates: [lon + radius * Math.cos(angle), lat + radius * Math.sin(angle)],
        },
      };
    }),
  };
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

export function filterGeoJsonUntilYear(
  collection: GeoJsonFeatureCollection,
  untilYear: number,
): GeoJsonFeatureCollection {
  return {
    type: "FeatureCollection",
    features: collection.features.filter((feature) => {
      const year = extractYear(String(feature.properties.start_time ?? ""));
      if (year == null) return true;
      return year <= untilYear;
    }),
  };
}

export function filterTimelineUntilYear(
  events: TimelineEvent[],
  untilYear: number,
): TimelineEvent[] {
  return events.filter((event) => {
    const year = extractYear(event.start_time);
    if (year == null) return true;
    return year <= untilYear;
  });
}

function yearsOfType(events: TimelineEvent[], eventType: string): number[] {
  return events
    .filter((event) => event.event_type === eventType)
    .map(eventYear)
    .filter((year): year is number => year != null);
}

export function buildYearBounds(events: TimelineEvent[]): { min: number; max: number } {
  const years = events.map(eventYear).filter((year): year is number => year != null);
  if (years.length === 0) return { min: 1800, max: new Date().getFullYear() };

  const births = yearsOfType(events, "birth");
  const deaths = yearsOfType(events, "death");
  const min = births.length ? Math.min(...births) : Math.min(...years);
  const deathAfterBirth = deaths.filter((year) => year >= min);
  const max = deathAfterBirth.length
    ? Math.min(...deathAfterBirth)
    : Math.max(...years);
  return { min, max: Math.max(min, max) };
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
