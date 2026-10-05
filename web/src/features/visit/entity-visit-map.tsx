// web/src/features/visit/entity-visit-map.tsx
import { useEffect, useMemo, useState } from "react";
import type { Map } from "maplibre-gl";
import { MapLegend } from "@/components/map/map-legend";
import {
  attachLegendKeys,
  eventMatchesLegendFilter,
  legendKeyForEventType,
  LEGEND_ORDER,
  type LegendKey,
} from "@/lib/event-legend";
import { spreadStackedMapPoints } from "@/lib/geo";
import { fetchVisitHeritageGeojson, fetchVisitNowGeojson } from "@/lib/entity-views";
import { MapCanvas } from "@/components/map/map-canvas";
import { MapSourceManager } from "@/components/map/map-source-manager";
import { MapLayers } from "@/components/map/map-layers";
import { MapInteractions } from "@/components/map/map-interactions";
import type { GeoJsonFeatureCollection } from "@/lib/api";
import type { TalariaFeatureCollection } from "@/lib/schemas/geojson";

export function EntityVisitMap({
  id,
  selected,
  focus,
  onSelect,
}: {
  id: string;
  selected?: string;
  focus?: { eventId: string; lat: number; lon: number };
  onSelect: (id: string) => void;
}) {
  const [map, setMap] = useState<Map | null>(null);
  const [data, setData] = useState<TalariaFeatureCollection>();
  const [keys, setKeys] = useState<LegendKey[]>([]);
  const presentKeys = useMemo(
    () => [...new Set((data?.features ?? []).map((f) => legendKeyForEventType(String(f.properties.event_type))))],
    [data],
  );
  const visible = useMemo(
    () =>
      data
        ? {
            ...data,
            features: data.features.filter((f) =>
              eventMatchesLegendFilter(String(f.properties.event_type), keys),
            ),
          }
        : undefined,
    [data, keys],
  );
  const [error, setError] = useState("");

  useEffect(() => {
    setData(undefined);
    setKeys([]);
  }, [id]);

  useEffect(() => {
    if (!map) return;
    let controller: AbortController | undefined;
    let timer: ReturnType<typeof setTimeout>;
    const load = () => {
      controller?.abort();
      controller = new AbortController();
      const signal = controller.signal;
      const b = map.getBounds();
      const crossesWorld = b.getWest() < -180 || b.getEast() > 180;
      const bbox = [
        crossesWorld ? -180 : b.getWest(),
        Math.max(-90, b.getSouth()),
        crossesWorld ? 180 : b.getEast(),
        Math.min(90, b.getNorth()),
      ].join(",");
      setError("");
      void Promise.all([
        fetchVisitHeritageGeojson(id, { bbox, signal }),
        fetchVisitNowGeojson(id, { bbox, signal }),
      ])
        .then(([heritage, now]) => {
          if (signal.aborted) return;
          const merged: GeoJsonFeatureCollection = {
            type: "FeatureCollection",
            features: [...heritage.features, ...now.features],
          };
          setData(
            spreadStackedMapPoints(attachLegendKeys(merged)) as TalariaFeatureCollection,
          );
        })
        .catch((e: unknown) => {
          if (!signal.aborted) setError(String(e));
        });
    };
    const schedule = () => {
      clearTimeout(timer);
      timer = setTimeout(load, 200);
    };
    load();
    map.on("moveend", schedule);
    return () => {
      clearTimeout(timer);
      controller?.abort();
      map.off("moveend", schedule);
    };
  }, [id, map]);

  useEffect(() => {
    if (!map || !focus) return;
    const zoom = Math.max(map.getZoom(), 7.5);
    map.flyTo({
      center: [focus.lon, focus.lat],
      zoom,
      duration: 1100,
      essential: true,
    });
  }, [map, focus?.eventId, focus?.lat, focus?.lon]);

  return (
    <div className="v3-map-layout v3-map-layout--visit">
      <div className="v3-map v3-map--visit">
        <MapCanvas onReady={setMap} />
        <MapSourceManager map={map} data={visible} />
        <MapLayers map={map} data={visible} selectedEventId={selected} />
        <MapInteractions map={map} onSelectEvent={onSelect} />
        {error && (
          <p role="alert" className="v3-map-error">
            {error}
          </p>
        )}
      </div>
      <div className="v3-map-legend">
        <MapLegend
          presentKeys={presentKeys.length ? presentKeys : LEGEND_ORDER}
          selectedKeys={keys}
          onToggleKey={(key) =>
            setKeys((old) => (old.includes(key) ? old.filter((k) => k !== key) : [...old, key]))
          }
          onClear={() => setKeys([])}
        />
      </div>
    </div>
  );
}
