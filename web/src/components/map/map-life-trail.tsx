// web/src/components/map/map-life-trail.tsx
import { useEffect } from "react";
import type { GeoJSONSource, Map } from "maplibre-gl";

export function MapLifeTrail({
  map,
  segments,
}: {
  map: Map | null;
  segments: [number, number][][];
}) {
  useEffect(() => {
    if (!map || typeof map.getSource !== "function") return;
    const data = {
      type: "Feature" as const,
      geometry: { type: "MultiLineString" as const, coordinates: segments },
      properties: {},
    };
    const sync = () => {
      if (!map.isStyleLoaded()) return;
      const existing = map.getSource("life-trail") as GeoJSONSource | undefined;
      if (!existing) {
        map.addSource("life-trail", { type: "geojson", data });
        const before = map.getLayer("unclustered-events") ? "unclustered-events" : undefined;
        map.addLayer({
          id: "life-trail",
          type: "line",
          source: "life-trail",
          paint: {
            "line-color": "#B96346",
            "line-width": 2.2,
            "line-opacity": 0.8,
            "line-dasharray": [1.1, 1.1],
          },
        }, before);
        return;
      }
      existing.setData(data);
    };
    if (map.isStyleLoaded()) sync();
    map.on("style.load", sync);
    return () => {
      map.off("style.load", sync);
    };
  }, [map, segments]);
  return null;
}
