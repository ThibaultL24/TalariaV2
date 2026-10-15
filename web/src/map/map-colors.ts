// web/src/map/map-colors.ts
export { MAP_LAYER_PALETTE, mapLayerPalette, type MapLayerPalette } from "./map-theme";

/** @deprecated Use mapLayerPalette(isDark) — kept for existing imports */
export const MAP_LAYER_COLORS_DARK = {
  cluster: "#45A6C4",
  eventLow: "#1a5560",
  eventMid: "#3a8f9a",
  eventHigh: "#72c4da",
  anecdote: "#e8b84a",
  marble: "#F2EEE5",
  pointStroke: "#08151C",
  accentStrong: "#F07A45",
} as const;
