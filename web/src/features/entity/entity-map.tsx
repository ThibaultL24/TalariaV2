import { useEffect, useMemo, useState } from "react";
import type { Map } from "maplibre-gl";
import { MapLegend } from "@/components/map/map-legend";
import { attachLegendKeys, eventMatchesLegendFilter, legendKeyForEventType, type LegendKey } from "@/lib/event-legend";
import { spreadStackedMapPoints } from "@/lib/geo";
import { MapCanvas } from "@/components/map/map-canvas";
import { MapSourceManager } from "@/components/map/map-source-manager";
import { MapLayers } from "@/components/map/map-layers";
import { MapInteractions } from "@/components/map/map-interactions";
import { getEntityView, type MapPage } from "@/lib/entity-views";
import type { TalariaFeatureCollection } from "@/lib/schemas/geojson";
export function EntityMap({
  id,
  filters,
  selected,
  onSelect,
}: {
  id: string;
  filters: string;
  selected?: string;
  onSelect: (id: string) => void;
}) {
  const [map, setMap] = useState<Map | null>(null);
  const [data, setData] = useState<TalariaFeatureCollection>();
  const [keys, setKeys] = useState<LegendKey[]>([]);
  const presentKeys = useMemo(() => [...new Set((data?.features ?? []).map(f => legendKeyForEventType(String(f.properties.event_type))))], [data]);
  const visible = useMemo(() => data ? { ...data, features: data.features.filter(f => eventMatchesLegendFilter(String(f.properties.event_type), keys)) } : undefined, [data, keys]);
  const [error, setError] = useState("");
  useEffect(() => {
    if (!map) return;
    let controller: AbortController | undefined;
    let timer: ReturnType<typeof setTimeout>;
    const load = () => {
      controller?.abort();
      controller = new AbortController();
      const signal = controller.signal;
      const b = map.getBounds();
      const params = new URLSearchParams(filters);
      // A viewport crossing the antimeridian uses the complete longitude range.
      const crossesWorld = b.getWest() < -180 || b.getEast() > 180;
      params.set(
        "bbox",
        [
          crossesWorld ? -180 : b.getWest(),
          Math.max(-90, b.getSouth()),
          crossesWorld ? 180 : b.getEast(),
          Math.min(90, b.getNorth()),
        ].join(","),
      );
      params.set("limit", "500");
      params.delete("cursor");
      setError("");
      setData(undefined);
      void (async () => {
        const features: MapPage["features"] = [];
        do {
          const page = await getEntityView<MapPage>(
            id,
            "events",
            params,
            signal,
          );
          features.push(...page.features);
          if (!signal.aborted)
            setData(spreadStackedMapPoints(attachLegendKeys({
              type: "FeatureCollection",
              features: [...features],
            })) as TalariaFeatureCollection);
          if (!page.pagination.next_cursor) break;
          params.set("cursor", page.pagination.next_cursor);
        } while (!signal.aborted);
      })().catch((e: unknown) => {
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
  }, [id, filters, map]);
  return (
    <div className="v3-map">
      <MapCanvas onReady={setMap} />
      <MapSourceManager map={map} data={visible} />
      <MapLayers map={map} data={visible} selectedEventId={selected} />
      <MapInteractions map={map} onSelectEvent={onSelect} />
      <div className="v3-map-legend"><MapLegend presentKeys={presentKeys} selectedKeys={keys} onToggleKey={(key) => setKeys(old => old.includes(key) ? old.filter(k => k !== key) : [...old, key])} onClear={() => setKeys([])} /></div>
      {error && (
        <p role="alert" className="v3-map-error">
          {error}
        </p>
      )}
    </div>
  );
}
