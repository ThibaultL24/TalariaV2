// src/styles/map-style-antique.js
/**
 * Style raster "antiquité méditerranéenne" pour MapLibre.
 * Objectif: ambiance chaleureuse sans filtre agressif global.
 */
export const ANTIQUE_MAP_STYLE = {
  version: 8,
  // openmaptiles.org peut renvoyer un PBF incompatible (« Unimplemented type: 4 ») selon MapLibre.
  glyphs: "https://tiles.openfreemap.org/fonts/{fontstack}/{range}.pbf",
  sources: {
    osm: {
      type: "raster",
      tiles: ["https://tile.openstreetmap.org/{z}/{x}/{y}.png"],
      tileSize: 256,
      attribution: "© OpenStreetMap contributors",
    },
  },
  layers: [
    {
      id: "paper-background",
      type: "background",
      paint: {
        "background-color": "#F8EFD9",
      },
    },
    {
      id: "osm-raster-antique",
      type: "raster",
      source: "osm",
      paint: {
        "raster-opacity": 0.96,
        "raster-saturation": -0.12,
        "raster-contrast": 0.03,
        "raster-brightness-min": 0.24,
        "raster-brightness-max": 0.98,
        "raster-hue-rotate": 6,
      },
    },
  ],
};

