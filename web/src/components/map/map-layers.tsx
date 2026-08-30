// src/components/map/MapLayers.tsx

import { useEffect } from "react";
import type { Map } from "maplibre-gl";
import type { TalariaFeatureCollection } from "@/lib/schemas/geojson";

interface MapLayersProps {
  map: Map | null;
  /** Same GeoJSON as MapSourceManager — type filtering is applied in ExplorerPage before upload. */
  data?: TalariaFeatureCollection;
  selectedEventId?: string;
}

export function MapLayers({ map, data, selectedEventId }: MapLayersProps) {
  useEffect(() => {
    if (!map) return;

    const applySelected = () => {
      if (!map.getLayer("selected-event")) return;

      if (selectedEventId) {
        map.setFilter("selected-event", ["==", ["id"], selectedEventId]);
        if (map.getLayer("selected-anecdote")) {
          map.setFilter("selected-anecdote", ["==", ["id"], selectedEventId]);
        }
      } else {
        map.setFilter("selected-event", ["==", ["id"], ""]);
        if (map.getLayer("selected-anecdote")) {
          map.setFilter("selected-anecdote", ["==", ["id"], ""]);
        }
      }
    };

    if (map.isStyleLoaded()) applySelected();
    map.on("load", applySelected);
    return () => {
      map.off("load", applySelected);
    };
  }, [map, data, selectedEventId]);

  return null;
}
