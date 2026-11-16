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

// Meters on the ground become pixels at the current zoom. 156543 m/px is the zoom-0 equator.
const referenceRadius = [
  "/",
  ["*", ["to-number", ["get", "uncertainty_radius_m"]], ["^", 2, ["zoom"]]],
  [
    "*",
    156543.03392,
    ["max", 0.2, ["cos", ["*", ["to-number", ["get", "latitude"]], 0.017453292519943295]]],
  ],
];

export const referenceAreaLayer: CircleLayerSpecification = {
  id: "reference-areas",
  type: "circle",
  source: "events",
  filter: ["in", ["get", "coord_precision_level"], ["literal", ["city", "region", "country"]]],
  paint: {
    "circle-radius": referenceRadius as unknown as number,
    "circle-color": L.pointStroke,
    "circle-opacity": 0.16,
    "circle-stroke-width": 1.25,
    "circle-stroke-color": L.pointStroke,
    "circle-stroke-opacity": 0.85,
  },
};

export const referenceAreaLayerDark: CircleLayerSpecification = {
  ...referenceAreaLayer,
  paint: {
    ...referenceAreaLayer.paint,
    "circle-color": D.pointStroke,
    "circle-stroke-color": D.pointStroke,
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
  referenceAreaLayer: CircleLayerSpecification;
  clustersLayer: CircleLayerSpecification;
  clusterCountLayer: SymbolLayerSpecification;
  unclusteredEventsLayer: CircleLayerSpecification;
  selectedEventLayer: CircleLayerSpecification;
  anecdotesLayer: CircleLayerSpecification;
  selectedAnecdoteLayer: CircleLayerSpecification;
} {
  if (isDark) {
    return {
      referenceAreaLayer: referenceAreaLayerDark,
      clustersLayer: clustersLayerDark,
      clusterCountLayer: clusterCountLayerDark,
      unclusteredEventsLayer: unclusteredEventsLayerDark,
      selectedEventLayer: selectedEventLayerDark,
      anecdotesLayer: anecdotesLayerDark,
      selectedAnecdoteLayer: selectedAnecdoteLayerDark,
    };
  }
  return {
    referenceAreaLayer,
    clustersLayer,
    clusterCountLayer,
    unclusteredEventsLayer,
    selectedEventLayer,
    anecdotesLayer,
    selectedAnecdoteLayer,
  };
}
