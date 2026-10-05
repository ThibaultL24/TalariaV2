// src/features/map/styles/eventLayers.ts

import type { CircleLayerSpecification, SymbolLayerSpecification } from "maplibre-gl";
import { MAP_LAYER_PALETTE } from "@/map/map-theme";
import { mapLibreLegendColorExpr } from "@/lib/event-legend";

const L = MAP_LAYER_PALETTE.light;
const D = MAP_LAYER_PALETTE.dark;
export const clustersLayer: CircleLayerSpecification = {
  id: "clusters",
  type: "circle",
  source: "events",
  filter: ["has", "point_count"],
  paint: {
    "circle-radius": ["step", ["get", "point_count"], 16, 25, 22, 100, 30],
    "circle-opacity": 0.92,
    "circle-color": L.cluster,
    "circle-stroke-width": 1.5,
    "circle-stroke-color": L.pointStroke,
  },
};

export const clusterCountLayer: SymbolLayerSpecification = {
  id: "cluster-count",
  type: "symbol",
  source: "events",
  filter: ["has", "point_count"],
  layout: {
    "text-field": ["get", "point_count_abbreviated"],
    "text-size": 12,
    // Fontstack must match `glyphs` in map style (OpenFreeMap serves Noto Sans).
    "text-font": ["Noto Sans Regular"],
  },
  paint: {
    "text-color": L.countText,
  },
};

export const unclusteredEventsLayer: CircleLayerSpecification = {
  id: "unclustered-events",
  type: "circle",
  source: "events",
  paint: {
    "circle-radius": [
      "interpolate",
      ["linear"],
      ["coalesce", ["get", "confidence"], ["get", "confidence_score"], 0.5],
      0,
      8,
      1,
      14,
    ],
    "circle-color": mapLibreLegendColorExpr() as unknown as string,
    "circle-stroke-width": 1.5,
    "circle-stroke-color": L.pointStroke,
    "circle-opacity": 0.95,
  },
};

export const selectedEventLayer: CircleLayerSpecification = {
  id: "selected-event",
  type: "circle",
  source: "events",
  filter: ["==", ["id"], ""],
  paint: {
    "circle-radius": 13,
    "circle-color": L.selected,
    "circle-stroke-width": 3,
    "circle-stroke-color": L.selectedStroke,
    "circle-opacity": L.selectedFillOpacity,
  },
};

/** Mode sombre — Aegean Night */
export const clustersLayerDark: CircleLayerSpecification = {
  ...clustersLayer,
  paint: {
    "circle-radius": ["step", ["get", "point_count"], 16, 25, 22, 100, 30],
    "circle-opacity": 0.95,
    "circle-color": D.cluster,
    "circle-stroke-width": 1.5,
    "circle-stroke-color": D.pointStroke,
  },
};

export const clusterCountLayerDark: SymbolLayerSpecification = {
  ...clusterCountLayer,
  layout: {
    ...clusterCountLayer.layout,
    "text-font": ["Noto Sans Regular"],
  },
  paint: {
    "text-color": D.countText,
  },
};

/** Confiance → teinte : cyan foncé → cyan moyen → accent mer lumineux. */
export const unclusteredEventsLayerDark: CircleLayerSpecification = {
  ...unclusteredEventsLayer,
  paint: {
    "circle-radius": [
      "interpolate",
      ["linear"],
      ["coalesce", ["get", "confidence"], ["get", "confidence_score"], 0.5],
      0,
      8,
      1,
      14,
    ],
    "circle-color": mapLibreLegendColorExpr() as unknown as string,
    "circle-stroke-width": 1.5,
    "circle-stroke-color": D.pointStroke,
    "circle-opacity": 0.98,
  },
};

export const selectedEventLayerDark: CircleLayerSpecification = {
  ...selectedEventLayer,
  paint: {
    "circle-radius": 13,
    "circle-color": D.selected,
    "circle-stroke-width": 3,
    "circle-stroke-color": D.selectedStroke,
    "circle-opacity": D.selectedFillOpacity,
  },
};

export const anecdotesLayer: CircleLayerSpecification = {
  id: "anecdotes",
  type: "circle",
  source: "anecdotes",
  paint: {
    "circle-radius": 10,
    "circle-color": mapLibreLegendColorExpr() as unknown as string,
    "circle-stroke-width": 1.5,
    "circle-stroke-color": L.pointStroke,
    "circle-opacity": 0.95,
  },
};

export const anecdotesLayerDark: CircleLayerSpecification = {
  ...anecdotesLayer,
  paint: {
    "circle-radius": 10,
    "circle-color": mapLibreLegendColorExpr() as unknown as string,
    "circle-stroke-width": 1.5,
    "circle-stroke-color": D.pointStroke,
    "circle-opacity": 0.98,
  },
};

export const selectedAnecdoteLayer: CircleLayerSpecification = {
  id: "selected-anecdote",
  type: "circle",
  source: "anecdotes",
  filter: ["==", ["id"], ""],
  paint: {
    "circle-radius": 14,
    "circle-color": L.anecdoteSelected,
    "circle-stroke-width": 3,
    "circle-stroke-color": L.selectedStroke,
    "circle-opacity": L.selectedFillOpacity,
  },
};

export const selectedAnecdoteLayerDark: CircleLayerSpecification = {
  ...selectedAnecdoteLayer,
  paint: {
    "circle-radius": 14,
    "circle-color": D.anecdoteSelected,
    "circle-stroke-width": 3,
    "circle-stroke-color": D.selectedStroke,
    "circle-opacity": D.selectedFillOpacity,
  },
};

export function getExplorerEventLayers(isDark: boolean): {
  clustersLayer: CircleLayerSpecification;
  clusterCountLayer: SymbolLayerSpecification;
  unclusteredEventsLayer: CircleLayerSpecification;
  selectedEventLayer: CircleLayerSpecification;
  anecdotesLayer: CircleLayerSpecification;
  selectedAnecdoteLayer: CircleLayerSpecification;
} {
  if (isDark) {
    return {
      clustersLayer: clustersLayerDark,
      clusterCountLayer: clusterCountLayerDark,
      unclusteredEventsLayer: unclusteredEventsLayerDark,
      selectedEventLayer: selectedEventLayerDark,
      anecdotesLayer: anecdotesLayerDark,
      selectedAnecdoteLayer: selectedAnecdoteLayerDark,
    };
  }
  return {
    clustersLayer,
    clusterCountLayer,
    unclusteredEventsLayer,
    selectedEventLayer,
    anecdotesLayer,
    selectedAnecdoteLayer,
  };
}
