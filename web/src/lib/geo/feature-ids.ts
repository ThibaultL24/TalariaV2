// web/src/lib/geo/feature-ids.ts
import type { TalariaFeature } from "@/lib/schemas/geojson";

export function getFeatureId(feature: TalariaFeature): string {
  if (feature.id != null && String(feature.id).trim()) {
    return String(feature.id);
  }
  const props = feature.properties ?? {};
  const pid = props.id ?? props.event_id;
  if (pid != null && String(pid).trim()) {
    return String(pid);
  }
  return crypto.randomUUID();
}

export function pickBestFeatureIdForEventDetail(features: TalariaFeature[]): string | null {
  for (const feature of features) {
    const id = getFeatureId(feature);
    if (id) return id;
  }
  return null;
}
