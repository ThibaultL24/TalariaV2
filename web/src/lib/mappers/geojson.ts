// web/src/lib/mappers/geojson.ts
import type { TalariaFeatureCollection } from "@/lib/schemas/geojson";
import { getFeatureId } from "@/lib/geo/feature-ids";

export function ensureFeatureIds(collection: TalariaFeatureCollection): TalariaFeatureCollection {
  return {
    type: "FeatureCollection",
    features: collection.features.map((feature) => {
      const id = getFeatureId(feature);
      return { ...feature, id, properties: { ...feature.properties, id } };
    }),
  };
}
