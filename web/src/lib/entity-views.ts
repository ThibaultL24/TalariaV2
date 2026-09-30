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
