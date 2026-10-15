// web/src/lib/map-preview.ts
import { eventTypeLabel } from "@/lib/event-taxonomy";
import { localizedEventTitle } from "@/lib/localize-event-copy";
import type { TalariaFeature } from "@/lib/schemas/geojson";
import type { AppLocale } from "@/stores/locale-store";

export interface MapEventPreview {
  id: string;
  title: string;
  when: string;
  category: string;
  summary?: string;
}

function asText(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

export function isClusterFeature(feature: TalariaFeature): boolean {
  const count = feature.properties?.point_count;
  return typeof count === "number" && count > 1;
}

function typedWhen(properties: Record<string, unknown>, unknownLabel: string): string {
  const time = properties.time;
  if (time && typeof time === "object") {
    const record = time as { surface?: unknown; kind?: unknown; start?: unknown };
    const surface = asText(record.surface);
    if (surface) return surface;
    if (record.kind === "unknown") return unknownLabel;
    const start = asText(record.start);
    if (start) return start.slice(0, 10);
  }
  const startTime = asText(properties.start_time);
  if (startTime) return startTime.slice(0, 4);
  return unknownLabel;
}

export function previewFromFeature(
  feature: TalariaFeature,
  locale: AppLocale,
  unknownDate: string,
): MapEventPreview | null {
  if (isClusterFeature(feature)) return null;
  const properties = feature.properties ?? {};
  const id = asText(properties.id) || asText(properties.event_id) || asText(feature.id);
  if (!id) return null;
  const eventType = asText(properties.event_type) || "historical_fact";
  const time = properties.time;
  const typedTime =
    time && typeof time === "object" && "kind" in time
      ? (time as { kind: "exact" | "range" | "approx" | "unknown"; start?: string | null })
      : undefined;
  const title = localizedEventTitle(
    {
      title: asText(properties.title),
      event_type: eventType,
      place_label: asText(properties.place_label) || null,
      start_time: asText(properties.start_time) || null,
      time: typedTime,
    },
    locale,
  );
  const summary = asText(properties.summary);
  return {
    id,
    title: title || eventTypeLabel(eventType, locale),
    when: typedWhen(properties, unknownDate),
    category: eventTypeLabel(eventType, locale),
    summary: summary ? summary.slice(0, 140) : undefined,
  };
}

export function escapePreviewText(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
