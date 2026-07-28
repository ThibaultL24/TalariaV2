// src/components/map/MapSourceManager.tsx

import { useEffect } from "react";
import type { Map, GeoJSONSource, LayerSpecification } from "maplibre-gl";
import type { TalariaFeatureCollection } from "@/lib/schemas/geojson";
import { ensureFeatureIds } from "@/lib/mappers/geojson";
import { getExplorerEventLayers } from "@/map/event-layers";
import { useThemeStore } from "@/stores/theme-store";

interface MapSourceManagerProps {
  map: Map | null;
  data?: TalariaFeatureCollection;
}

const EMPTY_COLLECTION: TalariaFeatureCollection = {
  type: "FeatureCollection",
  features: [],
};

function tryAddLayer(map: Map, layer: LayerSpecification): void {
  if (map.getLayer(layer.id)) return;
  try {
    map.addLayer(layer);
  } catch (e) {
    console.warn(`[Map] skipped layer "${layer.id}"`, e);
  }
}

function ensureEventLayers(map: Map, isDark: boolean): void {
  const L = getExplorerEventLayers(isDark);
  tryAddLayer(map, L.clustersLayer);
  tryAddLayer(map, L.unclusteredEventsLayer);
  tryAddLayer(map, L.selectedEventLayer);
  tryAddLayer(map, L.clusterCountLayer);
}

export function MapSourceManager({ map, data }: MapSourceManagerProps) {
  const isDark = useThemeStore((s) => s.theme === "dark");

  useEffect(() => {
    if (!map) return;

    const sync = () => {
      if (!map.isStyleLoaded()) return;

      const collection = data ? ensureFeatureIds(data) : EMPTY_COLLECTION;
      const existing = map.getSource("events");

      if (!existing) {
        map.addSource("events", {
          type: "geojson",
          data: collection,
          cluster: true,
          clusterMaxZoom: 12,
          clusterRadius: 50,
        });
        ensureEventLayers(map, isDark);
      } else {
        const source = existing as GeoJSONSource;
        source.setData(collection);
        ensureEventLayers(map, isDark);
      }
    };

    if (map.isStyleLoaded()) {
      sync();
    }
    map.on("load", sync);
    map.on("style.load", sync);

    return () => {
      map.off("load", sync);
      map.off("style.load", sync);
    };
  }, [map, data, isDark]);

  return null;
}
