// web/src/lib/geo/feature-ids.ts
import type { TalariaFeature } from "@/lib/schemas/geojson";

function asId(value: unknown): string | null {
  if (value == null) return null;
  const text = String(value).trim();
  return text ? text : null;
}

/**
 * Event identity for detail / selection.
 * Prefer `properties.id` — MapLibre clustering does not preserve GeoJSON feature ids
 * and may replace them with ephemeral integers.
 */
export function getFeatureId(feature: TalariaFeature): string {
  const props = feature.properties ?? {};
  const fromProps = asId(props.id) ?? asId(props.event_id);
  if (fromProps) return fromProps;
  const fromFeature = asId(feature.id);
  if (fromFeature) return fromFeature;
  return crypto.randomUUID();
}

export function pickBestFeatureIdForEventDetail(features: TalariaFeature[]): string | null {
  for (const feature of features) {
    const id = getFeatureId(feature);
    if (id) return id;
  }
  return null;
}
