// web/src/components/map/map-interactions.tsx
import { useEffect } from "react";
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
      if (id) onSelectEvent(id);
    };

    let frame = 0;
    const publishPreview = (e: MapMouseEvent) => {
      if (!onPreview) return;
      const layers = existingHitLayers(map);
      if (layers.length === 0) {
        onPreview(null);
        return;
      }
      const hits = map.queryRenderedFeatures(e.point, { layers });
      const feature = hits[0];
      if (!feature) {
        onPreview(null);
        return;
      }
      const properties = feature.properties ?? {};
      const title = typeof properties.title === "string" ? properties.title : "";
      if (!title) {
        onPreview(null);
        return;
      }
      onPreview({
        id: String(properties.id ?? feature.id ?? title),
        title,
        when: typedTimeLabel(properties.time),
        place: typeof properties.place_label === "string" ? properties.place_label : undefined,
      });
    };

    const setPointerCursor = (e: MapMouseEvent) => {
      const layers = existingHitLayers(map);
      if (layers.length === 0) {
        map.getCanvas().style.cursor = "";
        onPreview?.(null);
        return;
      }
      const hits = map.queryRenderedFeatures(e.point, { layers });
      map.getCanvas().style.cursor = hits.length ? "pointer" : "";
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => publishPreview(e));
    };

    const clearCursor = () => {
      map.getCanvas().style.cursor = "";
      onPreview?.(null);
    };

    map.on("click", handleClick);
    map.on("mousemove", setPointerCursor);
    map.on("mouseout", clearCursor);

    return () => {
      cancelAnimationFrame(frame);
      map.off("click", handleClick);
      map.off("mousemove", setPointerCursor);
      map.off("mouseout", clearCursor);
      map.getCanvas().style.cursor = "";
      onPreview?.(null);
    };
  }, [map, onSelectEvent, onPreview]);

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
