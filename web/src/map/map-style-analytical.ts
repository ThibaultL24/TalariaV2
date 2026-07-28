// web/src/map/map-style-analytical.ts
import type { StyleSpecification } from "maplibre-gl";

export const ANALYTICAL_MAP_STYLE: StyleSpecification = {
  version: 8,
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
      },
    },
  ],
};
