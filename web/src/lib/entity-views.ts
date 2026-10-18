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
  options?: { signal?: AbortSignal; lang?: string },
): Promise<{ events: TimelineEvent[]; count: number }> {
  const params = new URLSearchParams();
  if (options?.lang) params.set("lang", options.lang);
  const response = await fetch(`/api/v1/entities/${encodeURIComponent(id)}/visit/heritage?${params}`, {
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
  options?: { bbox?: string; signal?: AbortSignal; lang?: string },
): Promise<GeoJsonFeatureCollection> {
  const params = new URLSearchParams();
  if (options?.bbox) params.set("bbox", options.bbox);
  if (options?.lang) params.set("lang", options.lang);
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

export interface VisitSite {
  id: string;
  wikidata_qid: string;
  label: string;
  kind: "building" | "museum" | "monument" | "memorial" | "burial_site" | "residence";
  relation_pid: "P547" | "P138" | "P119" | "P551";
  coordinates: { lat: number; lon: number };
  visitability: "unknown" | "verified_visitable" | "verified_closed" | "restricted";
  opening_hours?: string | null;
  hours_checked_at?: string | null;
  association_source_url?: string | null;
}

export async function fetchVisitSites(
  id: string,
  options?: { signal?: AbortSignal },
): Promise<{ sites: VisitSite[]; count: number }> {
  const response = await fetch(`/api/v1/entities/${encodeURIComponent(id)}/visit/sites`, {
    signal: options?.signal,
  });
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
  return response.json() as Promise<{ sites: VisitSite[]; count: number }>;
}

export async function fetchVisitSitesGeojson(
  id: string,
  options?: { bbox?: string; signal?: AbortSignal },
): Promise<GeoJsonFeatureCollection> {
  const params = new URLSearchParams();
  if (options?.bbox) params.set("bbox", options.bbox);
  params.set("limit", "500");
  const response = await fetch(
    `/api/v1/entities/${encodeURIComponent(id)}/visit/sites/geojson?${params}`,
    { signal: options?.signal },
  );
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
  return response.json() as Promise<GeoJsonFeatureCollection>;
}

export interface PoreiaItinerary {
  stops: Array<{ id: string; wikidata_qid: string; label: string; coordinates: { lat: number; lon: number } }>;
  legs: Array<{ from_site_id: string; to_site_id: string; straight_line_km: number; travel_minutes: null }>;
  routing: "straight_line_only" | "osrm";
  travel_mode?: "walking" | "cycling" | "driving";
  route_distance_km?: number;
  route_geometry?: {type:"LineString";coordinates:[number,number][]};
  optimized_for?: "geographic_proximity";
  travel_minutes: number | null;
  warning?: string;
  message?: string;
}
export async function fetchPoreiaItinerary(
  id: string, signal?: AbortSignal,
  destination?: { lat: number; lon: number; radiusKm: number },
  mode: "walking" | "cycling" | "driving" = "walking",
): Promise<PoreiaItinerary> {
  const params = new URLSearchParams({ max_stops: "8", mode });
  if (destination) {
    params.set("lat", String(destination.lat));
    params.set("lon", String(destination.lon));
    params.set("radius_km", String(destination.radiusKm));
  }
  const response = await fetch(`/api/v1/entities/${encodeURIComponent(id)}/visit/itinerary?${params}`, { signal });
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
  return response.json() as Promise<PoreiaItinerary>;
}

export interface PoreiaDestination {
  qid: string;
  label: string;
  description: string;
  coordinates: { lat: number; lon: number };
  source_url: string;
}
export async function findPoreiaDestinations(
  query: string, lang: "fr" | "en" = "fr", signal?: AbortSignal,
): Promise<PoreiaDestination[]> {
  const params = new URLSearchParams({q:query.trim(),lang});
  const response = await fetch(`/api/v1/visit/destinations?${params}`,{signal});
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
  const result = await response.json() as {items:PoreiaDestination[]};
  return result.items;
}
