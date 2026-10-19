// web/src/components/map/map-interactions.tsx
import { useEffect, useRef } from "react";
import type { Map, MapMouseEvent } from "maplibre-gl";
import { pickBestFeatureIdForEventDetail } from "@/lib/geo/feature-ids";
import type { TalariaFeature } from "@/lib/schemas/geojson";

interface MapInteractionsProps {
  map: Map | null;
  onSelectEvent: (eventId: string) => void;
  onPreview?: (preview: MapHoverPreview | null) => void;
}

export interface MapHoverPreview {
  id: string;
  title: string;
  when?: string;
  place?: string;
}

const EVENT_HIT_LAYERS = [
  "selected-anecdote",
  "anecdotes",
  "selected-event",
  "unclustered-events",
] as const;

function existingHitLayers(map: Map): string[] {
  return EVENT_HIT_LAYERS.filter((id) => Boolean(map.getLayer(id)));
}

export function MapInteractions({ map, onSelectEvent, onPreview }: MapInteractionsProps) {
  const callbacks = useRef({ onSelectEvent, onPreview });
  callbacks.current = { onSelectEvent, onPreview };
  useEffect(() => {
    if (!map) return;

    const handleClick = (e: MapMouseEvent) => {
      const layers = existingHitLayers(map);
      if (layers.length === 0) return;
      const hits = map.queryRenderedFeatures(e.point, { layers });
      if (hits.length === 0) return;
      const id = pickBestFeatureIdForEventDetail(
        hits.map((hit) => hit as unknown as TalariaFeature),
      );
      if (id) callbacks.current.onSelectEvent(id);
    };

    let frame = 0;
    let latestMove: MapMouseEvent | null = null;
    let lastPreview: MapHoverPreview | null = null;

    const publishPreview = (preview: MapHoverPreview | null) => {
      if (
        lastPreview?.id === preview?.id &&
        lastPreview?.title === preview?.title &&
        lastPreview?.when === preview?.when &&
        lastPreview?.place === preview?.place
      ) return;
      lastPreview = preview;
      callbacks.current.onPreview?.(preview);
    };

    const flushMove = () => {
      frame = 0;
      const move = latestMove;
      latestMove = null;
      if (!move) return;
      const layers = existingHitLayers(map);
      const hits = layers.length ? map.queryRenderedFeatures(move.point, { layers }) : [];
      map.getCanvas().style.cursor = hits.length ? "pointer" : "";
      const feature = hits[0];
      if (!feature) {
        publishPreview(null);
        return;
      }
      const properties = feature.properties ?? {};
      const title = typeof properties.title === "string" ? properties.title : "";
      if (!title) {
        publishPreview(null);
        return;
      }
      publishPreview({
        id: String(properties.id ?? feature.id ?? title),
        title,
        when: typedTimeLabel(properties.time),
        place: typeof properties.place_label === "string" ? properties.place_label : undefined,
      });
    };

    const handleMove = (e: MapMouseEvent) => {
      latestMove = e;
      if (!frame) frame = requestAnimationFrame(flushMove);
    };

    const clearCursor = () => {
      if (frame) cancelAnimationFrame(frame);
      frame = 0;
      latestMove = null;
      map.getCanvas().style.cursor = "";
      publishPreview(null);
    };

    map.on("click", handleClick);
    map.on("mousemove", handleMove);
    map.on("mouseout", clearCursor);

    return () => {
      if (frame) cancelAnimationFrame(frame);
      map.off("click", handleClick);
      map.off("mousemove", handleMove);
      map.off("mouseout", clearCursor);
      map.getCanvas().style.cursor = "";
      publishPreview(null);
    };
  }, [map]);

  return null;
}

function typedTimeLabel(time: unknown): string | undefined {
  if (!time || typeof time !== "object") return undefined;
  const value = time as { kind?: string; surface?: string; start?: string; end?: string };
  if (value.kind === "unknown") return undefined;
  if (value.surface) return value.surface;
  if (value.kind === "range") return `${value.start ?? "?"} – ${value.end ?? "?"}`;
  if (value.kind === "approx" && value.start) return `≈ ${value.start}`;
  return value.start || undefined;
}
