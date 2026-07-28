// web/src/lib/schemas/geojson.ts
export interface TalariaFeature {
  type: "Feature";
  id?: string | number;
  geometry: GeoJSON.Geometry;
  properties: Record<string, unknown>;
}

export interface TalariaFeatureCollection {
  type: "FeatureCollection";
  features: TalariaFeature[];
}
