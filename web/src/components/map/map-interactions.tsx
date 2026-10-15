// web/src/components/map/map-interactions.tsx
import { useEffect } from "react";
import type { Map, MapMouseEvent } from "maplibre-gl";
import { pickBestFeatureIdForEventDetail } from "@/lib/geo/feature-ids";
import type { TalariaFeature } from "@/lib/schemas/geojson";

interface MapInteractionsProps {
  map: Map | null;
  onSelectEvent: (eventId: string) => void;
}

const EVENT_HIT_LAYERS = [
  "selected-anecdote",
  "anecdotes",
  "selected-event",
  "unclustered-events",
] as const;

function existingHitLayers(map: Map): string[] {
  return EVENT_HIT_LAYERS.filter((id) => Boolean(map.getLayer(id)));
}

export function MapInteractions({ map, onSelectEvent }: MapInteractionsProps) {
  useEffect(() => {
    if (!map) return;

    const handleClick = (e: MapMouseEvent) => {
      const layers = existingHitLayers(map);
      if (layers.length === 0) return;
      const hits = map.queryRenderedFeatures(e.point, { layers });
      if (hits.length === 0) return;
      const id = pickBestFeatureIdForEventDetail(
        hits.map((hit) => hit as unknown as TalariaFeature),
      );
      if (id) onSelectEvent(id);
    };

    const setPointerCursor = (e: MapMouseEvent) => {
      const layers = existingHitLayers(map);
      if (layers.length === 0) {
        map.getCanvas().style.cursor = "";
        return;
      }
      const hits = map.queryRenderedFeatures(e.point, { layers });
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
