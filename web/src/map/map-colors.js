// src/components/map/map-colors.js
// Palette « Méditerranée antique » (parchemin + carte)

export const MAP_COLORS = {
  papyrus: "#F9F1DE",
  ink: "#2E2A22",
  sand: "#F2E3BE",
  stone: "#CFB68F",
  lapis: "#5b77be",
  bronze: "#B17A43",
  marble: "#FFF8E9",
  terracotta: "#CD7A45",
  violet: "#7D679D",
  sea: "#3B6F8A",
  malachite: "#6F8A63",
  /** @deprecated use bronze — kept for map-layers */
  accent: "#B17A43",
  /** @deprecated use lapis */
  accentStrong: "#5B7FA8",
  /** @deprecated use stone */
  accentSoft: "#CFB68F",
  surface: "#F9F1DE",
  textPrimary: "#2E2A22",
  textSecondary: "#5B6472",
  borderSubtle: "#CFB68F",
  borderStrong: "#B9A67E",
  success: "#6F8A63",
  warning: "#CD7A45",
  danger: "#CD7A45",
  /** Contour des points seuls (carte claire) — lisible sur fond parchemin */
  pointStroke: "#4a3728",
};

/** UI + popups en dark mode — tons gris / blanc sur fond sombre (carte N&B). */
export const MAP_COLORS_DARK = {
  papyrus: "#252a31",
  ink: "#e5e7eb",
  sand: "#374151",
  stone: "#4b5563",
  lapis: "#d1d5db",
  bronze: "#e5e7eb",
  marble: "#111827",
  terracotta: "#f87171",
  violet: "#9ca3af",
  sea: "#9ca3af",
  malachite: "#9ca3af",
  accent: "#d1d5db",
  accentStrong: "#9ca3af",
  accentSoft: "#4b5563",
  surface: "#252a31",
  textPrimary: "#e5e7eb",
  textSecondary: "#9ca3af",
  borderSubtle: "#4b5563",
  borderStrong: "#6b7280",
  success: "#9ca3af",
  warning: "#d1d5db",
  danger: "#f87171",
};

/** Couches MapLibre — cyan foncé Intuition ; confiance → cyan plus lumineux. */
export const MAP_LAYER_COLORS_DARK = {
  cluster: "#2d7a86",
  eventLow: "#1a5560",
  eventMid: "#3a8f9a",
  eventHigh: "#7ae8f5",
  marble: "#ffffff",
  pointStroke: "#061016",
  ink: "#061016",
  accent: "#4acdda",
  accentStrong: "#7ae8f5",
  lapis: "#2d7a86",
  bronze: "#4a9dad",
};
