import type { TimelineEvent, GeoJsonFeatureCollection } from "./api";

export interface EntityOverview {
  entity: { id: string; label: string; qid?: string | null };
  time_bounds: { from: number | null; to: number | null };
  stats: { timeline_events: number; mapped_events: number; places: number };
}
export interface Page {
  events: TimelineEvent[];
  count: number;
  pagination: { next_cursor: string | null };
}
export type MapPage = GeoJsonFeatureCollection & {
  pagination: { next_cursor: string | null };
};
export async function fetchVisitHeritage(
  id: string,
  options?: { signal?: AbortSignal },
): Promise<{ events: TimelineEvent[]; count: number }> {
  const response = await fetch(`/api/v1/entities/${encodeURIComponent(id)}/visit/heritage`, {
    signal: options?.signal,
  });
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
  return response.json() as Promise<{ events: TimelineEvent[]; count: number }>;
}

export interface VisitOpportunity {
  id: string;
  kind: string;
  title: string;
  summary?: string | null;
  venue_label?: string | null;
  starts_at?: string | null;
  ends_at?: string | null;
  canonical_url?: string | null;
  source_kind: string;
  source_record_id?: string;
  fetched_at?: string;
  coordinates?: { lat: number; lon: number };
}

export async function fetchVisitNow(
  id: string,
  options?: { from?: string; to?: string; radiusKm?: number; signal?: AbortSignal },
): Promise<{ opportunities: VisitOpportunity[]; count: number }> {
  const params = new URLSearchParams();
  if (options?.from) params.set("from", options.from);
  if (options?.to) params.set("to", options.to);
  if (options?.radiusKm != null) params.set("radius_km", String(options.radiusKm));
  const response = await fetch(
    `/api/v1/entities/${encodeURIComponent(id)}/visit/now?${params}`,
    { signal: options?.signal },
  );
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
  return response.json() as Promise<{ opportunities: VisitOpportunity[]; count: number }>;
}

export async function fetchVisitNowGeojson(
  id: string,
  options?: { bbox?: string; signal?: AbortSignal },
): Promise<GeoJsonFeatureCollection> {
  const params = new URLSearchParams();
  if (options?.bbox) params.set("bbox", options.bbox);
  params.set("limit", "500");
  const response = await fetch(
    `/api/v1/entities/${encodeURIComponent(id)}/visit/now/geojson?${params}`,
    { signal: options?.signal },
  );
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
  return response.json() as Promise<GeoJsonFeatureCollection>;
}

export async function fetchVisitHeritageGeojson(
  id: string,
  options?: { bbox?: string; signal?: AbortSignal },
): Promise<GeoJsonFeatureCollection> {
  const params = new URLSearchParams();
  if (options?.bbox) params.set("bbox", options.bbox);
  params.set("limit", "500");
  const response = await fetch(
    `/api/v1/entities/${encodeURIComponent(id)}/visit/heritage/geojson?${params}`,
    { signal: options?.signal },
  );
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
  return response.json() as Promise<GeoJsonFeatureCollection>;
}

export async function getEntityView<T>(
  id: string,
  view: string,
  params: URLSearchParams,
  signal?: AbortSignal,
): Promise<T> {
  const response = await fetch(
    `/api/v1/entities/${encodeURIComponent(id)}/${view}?${params}`,
    { signal },
  );
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
  return response.json() as Promise<T>;
}
export function eventDate(event: TimelineEvent): string {
  const time = event.time;
  if (!time || time.kind === "unknown") return "Date unknown";
  if (time.surface) return time.surface;
  if (time.kind === "range") return `${time.start ?? "?"} – ${time.end ?? "?"}`;
  return `${time.kind === "approx" ? "≈ " : ""}${time.start ?? "?"}`;
}
