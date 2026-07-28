// src/components/map/MapCanvas.tsx

import { useEffect, useRef } from "react";
import maplibregl, { type Map } from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import { ANTIQUE_MAP_STYLE } from "@/styles/map-style-antique";
import { ANALYTICAL_MAP_STYLE } from "@/styles/map-style-analytical";
import { useThemeStore } from "@/stores/theme-store";

interface MapCanvasProps {
  onReady?: (map: Map) => void;
}

function pickStyle(isDark: boolean): string | maplibregl.StyleSpecification {
  return (isDark ? ANALYTICAL_MAP_STYLE : ANTIQUE_MAP_STYLE) as
    | string
    | maplibregl.StyleSpecification;
}

export function MapCanvas({ onReady }: MapCanvasProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const mapRef = useRef<Map | null>(null);
  const theme = useThemeStore((s) => s.theme);
  const prevThemeRef = useRef<string | null>(null);

  useEffect(() => {
    if (!containerRef.current || mapRef.current) return;

    const isDark = useThemeStore.getState().theme === "dark";
    const map = new maplibregl.Map({
      container: containerRef.current,
      style: pickStyle(isDark),
      center: [2.3522, 46.2276],
      zoom: 4,
    });

    map.addControl(new maplibregl.NavigationControl(), "top-right");
    mapRef.current = map;
    prevThemeRef.current = useThemeStore.getState().theme;

    map.on("load", () => {
      onReady?.(map);
    });

    return () => {
      map.remove();
      mapRef.current = null;
      prevThemeRef.current = null;
    };
  }, [onReady]);

  useEffect(() => {
    const m = mapRef.current;
    if (!m) return;
    if (prevThemeRef.current === null) {
      prevThemeRef.current = theme;
      return;
    }
    if (prevThemeRef.current === theme) return;
    prevThemeRef.current = theme;
    m.setStyle(pickStyle(theme === "dark"));
  }, [theme]);

  return <div ref={containerRef} className="h-full w-full" />;
}
