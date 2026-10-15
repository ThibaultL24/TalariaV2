/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_INTUITION_ENV?: "testnet" | "mainnet";
}

declare module "@/styles/map-style-antique" {
  import type { StyleSpecification } from "maplibre-gl";
  export const ANTIQUE_MAP_STYLE: StyleSpecification;
}

