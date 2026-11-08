/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_INTUITION_ENV?: "testnet" | "mainnet";
  readonly VITE_TALARIA_LICENSED_HERO_VIDEO?: "true" | "false";
  readonly VITE_TALARIA_LICENSED_HERO_VIDEO_SRC?: string;
}

declare module "@/styles/map-style-antique" {
  import type { StyleSpecification } from "maplibre-gl";
  export const ANTIQUE_MAP_STYLE: StyleSpecification;
}

