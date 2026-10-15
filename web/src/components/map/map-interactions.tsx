// web/src/components/map/map-interactions.tsx
import { useEffect, useRef, useState } from "react";
import maplibregl, { type Map, type MapMouseEvent } from "maplibre-gl";
import { escapePreviewText, previewFromFeature, type MapEventPreview } from "@/lib/map-preview";
import type { TalariaFeature } from "@/lib/schemas/geojson";
import { useI18n } from "@/lib/i18n";

interface MapInteractionsProps {
  map: Map | null;
  onSelectEvent: (eventId: string) => void;
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

function pointerCanHover(): boolean {
  return window.matchMedia("(hover: hover) and (pointer: fine)").matches;
}

function previewHtml(preview: MapEventPreview): string {
  const summary = preview.summary
    ? `<p class="popup-apercu__summary">${escapePreviewText(preview.summary)}</p>`
    : "";
  return `<div class="popup-apercu" role="tooltip">
    <p class="popup-apercu__title">${escapePreviewText(preview.title)}</p>
    <p class="popup-apercu__meta">${escapePreviewText(preview.when)} · ${escapePreviewText(preview.category)}</p>
    ${summary}
  </div>`;
}

export function MapInteractions({ map, onSelectEvent }: MapInteractionsProps) {
  const { t, locale } = useI18n();
  const liveRef = useRef<HTMLParagraphElement>(null);
  const [sheet, setSheet] = useState<MapEventPreview | null>(null);
  const labels = useRef({ unknown: t.unknownDate, locale });
  labels.current = { unknown: t.unknownDate, locale };

  useEffect(() => {
    if (!map) return;
    const popup = new maplibregl.Popup({
      closeButton: false,
      closeOnClick: false,
      offset: 12,
      maxWidth: "240px",
      className: "map-event-preview",
    });
    let shownId = "";

    const hidePreview = () => {
      shownId = "";
      popup.remove();
      if (liveRef.current) liveRef.current.textContent = "";
    };

    const showPreview = (feature: TalariaFeature, point: { lng: number; lat: number }) => {
      const preview = previewFromFeature(feature, labels.current.locale, labels.current.unknown);
      if (!preview) {
        hidePreview();
        return;
      }
      if (preview.id !== shownId) {
        shownId = preview.id;
        popup.setHTML(previewHtml(preview));
        if (liveRef.current) {
          liveRef.current.textContent = `${preview.title}. ${preview.when}. ${preview.category}.`;
        }
      }
      popup.setLngLat(point).addTo(map);
    };

    const featureAt = (event: MapMouseEvent): TalariaFeature | null => {
      const layers = existingHitLayers(map);
      if (layers.length === 0) return null;
      const hits = map.queryRenderedFeatures(event.point, { layers });
      return (hits[0] as unknown as TalariaFeature | undefined) ?? null;
    };

    const handleClick = (event: MapMouseEvent) => {
      const feature = featureAt(event);
      if (!feature) return;
      const preview = previewFromFeature(feature, labels.current.locale, labels.current.unknown);
      if (!preview) return;
      if (!pointerCanHover()) {
        setSheet(preview);
        return;
      }
      onSelectEvent(preview.id);
    };

    const handleMove = (event: MapMouseEvent) => {
      const layers = existingHitLayers(map);
      const hits = layers.length ? map.queryRenderedFeatures(event.point, { layers }) : [];
      map.getCanvas().style.cursor = hits.length ? "pointer" : "";
      if (!pointerCanHover()) return;
      const feature = hits[0] as unknown as TalariaFeature | undefined;
      if (!feature) {
        hidePreview();
        return;
      }
      showPreview(feature, event.lngLat);
    };

    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") hidePreview();
    };

    map.on("click", handleClick);
    map.on("mousemove", handleMove);
    map.on("mouseout", hidePreview);
    map.getCanvas().addEventListener("keydown", onKey);

    return () => {
      map.off("click", handleClick);
      map.off("mousemove", handleMove);
      map.off("mouseout", hidePreview);
      map.getCanvas().removeEventListener("keydown", onKey);
      map.getCanvas().style.cursor = "";
      popup.remove();
    };
  }, [map, onSelectEvent]);

  return (
    <>
      <p ref={liveRef} className="map-preview-live" aria-live="polite" />
      {sheet ? (
        <aside className="map-preview-sheet" role="dialog" aria-label={sheet.title}>
          <p className="popup-apercu__title">{sheet.title}</p>
          <p className="popup-apercu__meta">
            {sheet.when} · {sheet.category}
          </p>
          {sheet.summary ? <p className="popup-apercu__summary">{sheet.summary}</p> : null}
          <div className="map-preview-sheet__actions">
            <button type="button" onClick={() => onSelectEvent(sheet.id)}>
              {t.openEventDetail}
            </button>
            <button type="button" onClick={() => setSheet(null)}>
              {t.close}
            </button>
          </div>
        </aside>
      ) : null}
    </>
  );
}
