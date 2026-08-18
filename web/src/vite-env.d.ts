/// <reference types="vite/client" />

declare module "@/styles/map-style-antique" {
  import type { StyleSpecification } from "maplibre-gl";
  export const ANTIQUE_MAP_STYLE: StyleSpecification;
}

declare module "@/styles/map-style-analytical" {
  import type { StyleSpecification } from "maplibre-gl";
  export const ANALYTICAL_MAP_STYLE: StyleSpecification;
}

declare module "@/components/map/map-colors" {
  export const MAP_LAYER_COLORS_DARK: {
    cluster: string;
    pointStroke: string;
    marble: string;
    accentStrong: string;
    eventLow: string;
    eventMid: string;
    eventHigh: string;
    anecdote: string;
  };
}
