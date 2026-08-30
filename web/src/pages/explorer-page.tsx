// web/src/pages/explorer-page.tsx
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Map } from "maplibre-gl";
import { EventDetailCard } from "@/components/detail/event-detail-card";
import { Navbar } from "@/components/layout/navbar";
import { ExplorerMapTimelineBar } from "@/components/map/explorer-map-timeline-bar";
import { MapCanvas } from "@/components/map/map-canvas";
import { MapInteractions } from "@/components/map/map-interactions";
import { MapLayers } from "@/components/map/map-layers";
import { MapLegend } from "@/components/map/map-legend";
import { MapSourceManager } from "@/components/map/map-source-manager";
import { EntitySearchBox } from "@/components/search/entity-search-box";
import { usePersonPicker } from "@/hooks/use-person-picker";
import {
  fetchGeoJson,
  fetchStatus,
  fetchTimeline,
  type GeoJsonFeatureCollection,
  type TimelineEvent,
} from "@/lib/api";
import {
  attachLegendKeys,
  legendKeyForEventType,
  type LegendKey,
} from "@/lib/event-legend";
import {
  boundsOfMapFeatures,
  buildYearBounds,
  buildYearHistogram,
  filterGeoJsonUntilYear,
  filterTimelineUntilYear,
  spreadStackedMapPoints,
} from "@/lib/geo";
import { useI18n } from "@/lib/i18n";
import type { TalariaFeatureCollection } from "@/lib/schemas/geojson";
import { useExplorerStore } from "@/stores/explorer-store";

const POLL_MS = 5000;
const INGEST_POLL_MS = 1500;
const LIVE_LIMIT = 2000;

function toMapCollection(data: GeoJsonFeatureCollection): TalariaFeatureCollection {
  return spreadStackedMapPoints(data) as TalariaFeatureCollection;
}

export function ExplorerPage() {
  const { t } = useI18n();
  const [map, setMap] = useState<Map | null>(null);
  const [allEvents, setAllEvents] = useState<TimelineEvent[]>([]);
  const [geojson, setGeojson] = useState<GeoJsonFeatureCollection | null>(null);
  const [loading, setLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [untilYear, setUntilYear] = useState<number | null>(null);
  const { suggestions, setSearchQuery, searchLoading, selectPerson, ingestBusy, error } =
    usePersonPicker();

  const { entityId, entityLabel, personFilter, selectedEventId, setSelectedEventId, closeDetail } =
    useExplorerStore();

  const hasEntity = Boolean(entityId || personFilter);

  useEffect(() => {
    fetchStatus().catch(() => undefined);
  }, []);

  useEffect(() => {
    if (!hasEntity) {
      setAllEvents([]);
      setGeojson(null);
      setUntilYear(null);
      return;
    }

    let cancelled = false;
    let inFlight = false;
    let first = true;
    setLoadError(null);

    const query = {
      entityId: entityId ?? undefined,
      person: personFilter ?? undefined,
      limit: LIVE_LIMIT,
    };

    async function load() {
      if (inFlight) return;
      inFlight = true;
      if (first && !ingestBusy) setLoading(true);
      try {
        const [timeline, mapData] = await Promise.all([
          fetchTimeline(query),
          fetchGeoJson(query),
        ]);
        if (cancelled) return;
        setAllEvents(timeline.events);
        setGeojson(attachLegendKeys(mapData));
        const bounds = buildYearBounds(timeline.events);
        setUntilYear((prev) => {
          if (prev == null || first) return bounds.max;
          return Math.min(Math.max(prev, bounds.min), bounds.max);
        });
        setLoadError(null);
      } catch (err) {
        if (!cancelled && first) {
          setLoadError(err instanceof Error ? err.message : "load failed");
        }
      } finally {
        inFlight = false;
        if (!cancelled && first) {
          setLoading(false);
          first = false;
        }
      }
    }

    load();
    const tick = window.setInterval(load, ingestBusy ? INGEST_POLL_MS : POLL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(tick);
    };
  }, [entityId, personFilter, hasEntity, ingestBusy]);

  useEffect(() => {
    if (!map) return;
    map.setPadding({ top: 12, bottom: 96, left: 8, right: 88 });
  }, [map]);

  const dataBounds = useMemo(() => buildYearBounds(allEvents), [allEvents]);
  const playhead = untilYear ?? dataBounds.max;

  const visibleEvents = useMemo(
    () => filterTimelineUntilYear(allEvents, playhead),
    [allEvents, playhead],
  );

  const visibleGeoJson = useMemo(() => {
    if (!geojson) return { type: "FeatureCollection" as const, features: [] };
    return filterGeoJsonUntilYear(geojson, playhead);
  }, [geojson, playhead]);

  const histogram = useMemo(
    () =>
      buildYearHistogram(allEvents).filter(
        (row) => row.year >= dataBounds.min && row.year <= dataBounds.max,
      ),
    [allEvents, dataBounds],
  );

  const presentKeys = useMemo(() => {
    const keys = new Set<LegendKey>();
    for (const event of allEvents) {
      keys.add(legendKeyForEventType(event.event_type));
    }
    return [...keys];
  }, [allEvents]);

  const selectedEvent = useMemo(() => {
    if (!selectedEventId) return null;
    const fromTimeline = allEvents.find((event) => event.id === selectedEventId);
    if (fromTimeline) return fromTimeline;
    const feature = geojson?.features.find((row) => {
      const props = row.properties ?? {};
      return String(props.id ?? props.event_id ?? row.id ?? "") === selectedEventId;
    });
    if (!feature) return null;
    const props = feature.properties ?? {};
    const coords = feature.geometry?.coordinates;
    return {
      id: selectedEventId,
      entity_id: String(props.entity_id ?? entityId ?? ""),
      person: String(props.person ?? entityLabel ?? ""),
      event_type: String(props.event_type ?? "unknown"),
      epistemic_status: String(props.epistemic_status ?? "attested"),
      title: String(props.title ?? "Event"),
      summary: (props.summary as string | null | undefined) ?? null,
      start_time: (props.start_time as string | null | undefined) ?? null,
      place_label: (props.place_label as string | null | undefined) ?? null,
      confidence: Number(props.confidence ?? 0.5),
      map_eligible: true,
      coordinates:
        Array.isArray(coords) && coords.length >= 2
          ? { lon: Number(coords[0]), lat: Number(coords[1]) }
          : null,
    };
  }, [allEvents, selectedEventId, geojson, entityId, entityLabel]);

  const handleSelectEvent = useCallback(
    (eventId: string) => {
      setSelectedEventId(eventId);
      const event = allEvents.find((item) => item.id === eventId);
      if (map && event?.coordinates) {
        map.flyTo({
          center: [event.coordinates.lon, event.coordinates.lat],
          zoom: 8,
          essential: true,
          padding: { top: 12, bottom: 160, left: 8, right: 8 },
        });
      }
    },
    [allEvents, map, setSelectedEventId],
  );

  useEffect(() => {
    if (!selectedEventId) return;
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") closeDetail();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [selectedEventId, closeDetail]);

  const mapData = hasEntity ? toMapCollection(visibleGeoJson) : undefined;
  const banner = error ?? loadError;
  const fittedEntity = useRef<string>("");

  useEffect(() => {
    const entityKey = entityId ?? personFilter ?? "";
    if (!map || !entityKey || !mapData?.features.length) return;
    const token = `${entityKey}:${ingestBusy ? "busy" : "idle"}`;
    if (fittedEntity.current === token) return;
    if (fittedEntity.current.startsWith(`${entityKey}:busy`) && ingestBusy) return;
    const box = boundsOfMapFeatures(mapData);
    if (!box) return;
    fittedEntity.current = token;
    map.fitBounds(box, {
      padding: { top: 56, bottom: 120, left: 24, right: 96 },
      maxZoom: 6,
      duration: 700,
    });
  }, [entityId, ingestBusy, map, mapData, personFilter]);

  return (
    <div className="app-shell map-shell flex h-screen flex-col">
      <Navbar
        center={
          <EntitySearchBox
            suggestions={suggestions}
            onSubmitQuery={setSearchQuery}
            onSelect={selectPerson}
            isLoading={searchLoading}
          />
        }
      />

      <main className="relative min-h-0 flex-1">
        <div className="absolute inset-0">
          <MapCanvas onReady={setMap} />
        </div>
        <MapSourceManager map={map} data={mapData} />
        <MapLayers map={map} data={mapData} selectedEventId={selectedEventId} />
        <MapInteractions map={map} onSelectEvent={handleSelectEvent} />

        {entityLabel ? (
          <div className="pointer-events-none absolute top-3 left-3 z-10 rounded-lg border border-(--map-panel-border) bg-(--color-bg-elevated)/80 px-3 py-1.5 text-sm font-medium backdrop-blur-sm">
            {entityLabel}
          </div>
        ) : null}

        {ingestBusy || loading ? (
          <div className="pointer-events-none absolute top-3 left-1/2 z-10 -translate-x-1/2 rounded-full border border-(--map-panel-border) bg-(--color-bg-elevated)/85 px-3 py-1 text-[11px] text-(--color-text-secondary) backdrop-blur-sm">
            {ingestBusy ? t.searchInProgress : t.loadingMap}
          </div>
        ) : null}

        {banner ? (
          <p className="absolute top-14 left-1/2 z-10 -translate-x-1/2 rounded-lg bg-red-950/80 px-3 py-1.5 text-sm text-red-200">
            {banner}
          </p>
        ) : null}

        {!hasEntity && !ingestBusy ? (
          <div className="pointer-events-none absolute top-1/3 left-1/2 z-10 w-[min(100%-2rem,24rem)] -translate-x-1/2 text-center text-sm text-(--color-text-secondary)">
            {t.emptySearch}
          </div>
        ) : null}

        {hasEntity && allEvents.length > 0 ? <MapLegend presentKeys={presentKeys} /> : null}

        {hasEntity && allEvents.length > 0 ? (
          <ExplorerMapTimelineBar
            bounds={dataBounds}
            untilYear={playhead}
            onUntilYearChange={setUntilYear}
            visibleCount={visibleEvents.length}
            totalCount={allEvents.length}
            yearHistogram={histogram}
          />
        ) : null}

        {selectedEventId && selectedEvent ? (
          <div
            className="fixed inset-0 z-50 flex items-end justify-center p-3 sm:items-center sm:p-6"
            role="presentation"
          >
            <button
              type="button"
              className="absolute inset-0 bg-black/45 backdrop-blur-sm"
              aria-label={t.closeDetail}
              onClick={closeDetail}
            />
            <div
              className="nebula-event-detail relative flex max-h-[min(88vh,760px)] w-full max-w-lg flex-col overflow-hidden rounded-xl"
              role="dialog"
              aria-modal="true"
              aria-labelledby="event-detail-card-title"
            >
              <EventDetailCard event={selectedEvent} onClose={closeDetail} offlineOnly={false} />
            </div>
          </div>
        ) : null}
      </main>
    </div>
  );
}
