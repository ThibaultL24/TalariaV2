// src/styles/map-style-analytical.js
/**
 * Carte mode sombre — fond noir, tuiles Carto dark, rendu proche noir & blanc (désaturation).
 */
export const ANALYTICAL_MAP_STYLE = {
  version: 8,
  // OpenFreeMap glyphs (demotiles font endpoint returns 404 as of 2026).
  glyphs: "https://tiles.openfreemap.org/fonts/{fontstack}/{range}.pbf",
  sources: {
    cartoDark: {
      type: "raster",
      tiles: [
        "https://a.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png",
        "https://b.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png",
      ],
      tileSize: 256,
      attribution: "© CARTO © OpenStreetMap",
    },
  },
  layers: [
    {
      id: "carto-dark-bg",
      type: "background",
      paint: { "background-color": "#000000" },
    },
    {
      id: "carto-dark-raster",
      type: "raster",
      source: "cartoDark",
      paint: {
        "raster-opacity": 1,
        "raster-saturation": -0.92,
        "raster-contrast": 0.06,
        "raster-brightness-min": 0,
        "raster-brightness-max": 1,
      },
    },
  ],
};
