// web/src/features/visit/visit-map.tsx
import { useEffect, useRef } from "react";
import type { GeoJSONSource, Map } from "maplibre-gl";
import { MapCanvas } from "@/components/map/map-canvas";

interface VisitFeatureCollection {
  type: "FeatureCollection";
  features: Array<{ type: "Feature"; properties?: { id?: string } }>;
}

interface VisitMapProps {
  geojson: VisitFeatureCollection;
  selectedId?: string;
  onSelect: (id: string) => void;
}
export function VisitMap({ geojson, selectedId, onSelect }: VisitMapProps) {
  const mapRef = useRef<Map | null>(null);

  useEffect(() => {
    const map = mapRef.current;
    if (!map?.isStyleLoaded()) return;
    const source = map.getSource("visit-sites") as GeoJSONSource | undefined;
    if (source) source.setData(geojson as never);
  }, [geojson]);

  useEffect(() => {
    const map = mapRef.current;
    if (!map?.getLayer("visit-sites")) return;
    map.setFilter("visit-sites-selected", ["==", ["get", "id"], selectedId ?? ""]);
  }, [selectedId]);

  function onReady(map: Map) {
    mapRef.current = map;
    map.on("load", () => {
      map.addSource("visit-sites", { type: "geojson", data: geojson as never, cluster: true, clusterRadius: 40 });
      map.addLayer({
        id: "visit-clusters",
        type: "circle",
        source: "visit-sites",
        filter: ["has", "point_count"],
        paint: {
          "circle-color": "#1f6f5b",
          "circle-radius": 16,
        },
      });
      map.addLayer({
        id: "visit-sites",
        type: "circle",
        source: "visit-sites",
        filter: ["!", ["has", "point_count"]],
        paint: {
          "circle-color": "#c4a15a",
          "circle-radius": 8,
          "circle-stroke-width": 2,
          "circle-stroke-color": "#fff",
        },
      });
      map.addLayer({
        id: "visit-sites-selected",
        type: "circle",
        source: "visit-sites",
        filter: ["==", ["get", "id"], selectedId ?? ""],
        paint: { "circle-color": "#8c2f39", "circle-radius": 11 },
      });
      map.on("click", "visit-sites", (event) => {
        const id = event.features?.[0]?.properties?.id;
        if (typeof id === "string") onSelect(id);
      });
    });
  }

  return (
    <div className="visit-map" data-testid="visit-map">
      <MapCanvas onReady={onReady} />
    </div>
  );
}
