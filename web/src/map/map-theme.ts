// web/src/map/map-theme.ts — MapLibre palettes & style selection (Aegean Day/Night)

import type { StyleSpecification } from "maplibre-gl";
import { ANTIQUE_MAP_STYLE } from "@/styles/map-style-antique";

export const OPENFREEMAP_DARK_STYLE = "https://tiles.openfreemap.org/styles/dark";

export const MAP_LAYER_PALETTE = {
  light: {
    background: "#F7F3EA",
    cluster: "#0F5F7A",
    clusterStroke: "#20313A",
    selected: "#E76F3C",
    selectedStroke: "#FFFCF6",
    selectedFillOpacity: 0.28,
    anecdoteSelected: "#E76F3C",
    countText: "#FFFCF6",
    pointStroke: "#20313A",
  },
  dark: {
    background: "#08151C",
    cluster: "#45A6C4",
    clusterStroke: "#08151C",
    selected: "#F07A45",
    selectedStroke: "#F2EEE5",
    selectedFillOpacity: 0.32,
    anecdoteSelected: "#F07A45",
    countText: "#F2EEE5",
    pointStroke: "#08151C",
  },
} as const;

export type MapLayerPalette = (typeof MAP_LAYER_PALETTE)[keyof typeof MAP_LAYER_PALETTE];

export function mapLayerPalette(isDark: boolean): MapLayerPalette {
  return isDark ? MAP_LAYER_PALETTE.dark : MAP_LAYER_PALETTE.light;
}

export function pickMapStyle(isDark: boolean): string | StyleSpecification {
  if (isDark) return OPENFREEMAP_DARK_STYLE;
  return ANTIQUE_MAP_STYLE as StyleSpecification;
}
