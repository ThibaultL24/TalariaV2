// src/components/map/MapInteractions.tsx

import { useEffect } from "react";
import type { Map, MapMouseEvent, GeoJSONSource } from "maplibre-gl";
import { pickBestFeatureIdForEventDetail } from "@/lib/geo/feature-ids";
import type { TalariaFeature } from "@/lib/schemas/geojson";

interface MapInteractionsProps {
  map: Map | null;
  onSelectEvent: (eventId: string) => void;
}

/** Top to bottom in the style stack — queryRenderedFeatures returns the topmost hit first. */
const EVENT_HIT_LAYERS = [
  "cluster-count",
  "selected-event",
  "unclustered-events",
  "clusters",
] as const;

async function expandClusterAtClick(map: Map, e: MapMouseEvent): Promise<boolean> {
  let cluster = map.queryRenderedFeatures(e.point, { layers: ["clusters"] })[0];
  if (!cluster) {
    cluster = map.queryRenderedFeatures(e.point, { layers: ["cluster-count"] })[0];
  }
  if (!cluster) return false;

  const clusterId = cluster.properties?.cluster_id;
  const source = map.getSource("events") as GeoJSONSource;
  if (!source?.getClusterExpansionZoom || clusterId == null) return false;

  try {
    const zoom = await source.getClusterExpansionZoom(Number(clusterId));
    const coords = (cluster.geometry as GeoJSON.Point).coordinates;
    map.easeTo({ center: coords as [number, number], zoom });
    return true;
  } catch {
    return false;
  }
}

export function MapInteractions({ map, onSelectEvent }: MapInteractionsProps) {
  useEffect(() => {
    if (!map) return;

    const handleClick = async (e: MapMouseEvent) => {
      const hits = map.queryRenderedFeatures(e.point, { layers: [...EVENT_HIT_LAYERS] });
      const top = hits[0];
      if (!top) return;

      const layerId = top.layer.id;

      if (layerId === "clusters" || layerId === "cluster-count") {
        await expandClusterAtClick(map, e);
        return;
      }

      if (layerId === "unclustered-events" || layerId === "selected-event") {
        const eventHits = hits.filter(
          (h) =>
            h.layer.id === "unclustered-events" || h.layer.id === "selected-event"
        );
        const id = pickBestFeatureIdForEventDetail(
          eventHits.map((h) => h as unknown as TalariaFeature)
        );
        if (id) onSelectEvent(id);
      }
    };

    const setPointerCursor = (e: MapMouseEvent) => {
      const hits = map.queryRenderedFeatures(e.point, { layers: [...EVENT_HIT_LAYERS] });
      map.getCanvas().style.cursor = hits.length ? "pointer" : "";
    };

    const clearCursor = () => {
      map.getCanvas().style.cursor = "";
    };

    map.on("click", handleClick);
    map.on("mousemove", setPointerCursor);
    map.on("mouseout", clearCursor);

    return () => {
      map.off("click", handleClick);
      map.off("mousemove", setPointerCursor);
      map.off("mouseout", clearCursor);
      map.getCanvas().style.cursor = "";
    };
  }, [map, onSelectEvent]);

  return null;
}
