// src/components/map/MapLayers.tsx

import { useEffect } from "react";
import type { Map, FilterSpecification } from "maplibre-gl";
import type { TalariaFeatureCollection } from "@/lib/schemas/geojson";

interface MapLayersProps {
  map: Map | null;
  /** Same GeoJSON as MapSourceManager — type filtering is applied in ExplorerPage before upload. */
  data?: TalariaFeatureCollection;
  selectedEventId?: string;
}

const UNCLUSTERED_ONLY: FilterSpecification = ["!", ["has", "point_count"]];

export function MapLayers({ map, data, selectedEventId }: MapLayersProps) {
  useEffect(() => {
    if (!map) return;

    const applyUnclustered = () => {
      if (!map.getLayer("unclustered-events")) return;
      map.setFilter("unclustered-events", UNCLUSTERED_ONLY);
    };

    if (map.isStyleLoaded()) applyUnclustered();
    map.on("load", applyUnclustered);
    return () => {
      map.off("load", applyUnclustered);
    };
  }, [map, data]);

  useEffect(() => {
    if (!map) return;

    const applySelected = () => {
      if (!map.getLayer("selected-event")) return;

      if (selectedEventId) {
        map.setFilter("selected-event", ["==", ["id"], selectedEventId]);
      } else {
        map.setFilter("selected-event", ["==", ["id"], ""]);
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
