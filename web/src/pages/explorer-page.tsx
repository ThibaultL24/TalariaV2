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
import {
  fetchGeoJson,
  fetchIngestJob,
  fetchStatus,
  fetchTimeline,
  searchEntities,
  startExplorerIngest,
  type GeoJsonFeatureCollection,
  type IngestJobResponse,
  type TimelineEvent,
} from "@/lib/api";
import {
  attachLegendKeys,
  legendKeyForEventType,
  type LegendKey,
} from "@/lib/event-legend";
import {
  buildYearBounds,
  buildYearHistogram,
  filterGeoJsonUntilYear,
  filterTimelineUntilYear,
} from "@/lib/geo";
import { useI18n } from "@/lib/i18n";
import type { SearchSuggestion } from "@/lib/schemas/entity";
import type { TalariaFeatureCollection } from "@/lib/schemas/geojson";
import { collapseToSinglePersonSuggestion } from "@/lib/search-suggestions";
import { useExplorerStore } from "@/stores/explorer-store";

const POLL_MS = 5000;
const INGEST_POLL_MS = 1500;
const INGEST_TIMEOUT_MS = 45 * 60 * 1000;
const LIVE_LIMIT = 2000;

async function pollIngestJob(
  jobId: string,
  onTick: (job: IngestJobResponse) => void,
): Promise<IngestJobResponse> {
  const deadline = Date.now() + INGEST_TIMEOUT_MS;
  while (Date.now() < deadline) {
    const job = await fetchIngestJob(jobId);
    onTick(job);
    if (job.status !== "running" && job.status !== "queued") return job;
    await new Promise((resolve) => setTimeout(resolve, INGEST_POLL_MS));
  }
  throw new Error("timeout");
}

function namesOverlap(left: string, right: string): boolean {
  const a = left.trim().toLowerCase();
  const b = right.trim().toLowerCase();
  return a.includes(b) || b.includes(a);
}

function preferDenseLocalAlias(
  item: SearchSuggestion,
  items: SearchSuggestion[],
): SearchSuggestion {
  if (!item.known_locally || !item.label) return item;
  const denser = items
    .filter(
      (row) =>
        row.known_locally &&
        row.entity_id &&
        row.label &&
        namesOverlap(row.label, item.label) &&
        (row.event_count ?? 0) > (item.event_count ?? 0),
    )
    .sort((a, b) => (b.event_count ?? 0) - (a.event_count ?? 0))[0];
  return denser ?? item;
}

function toMapCollection(data: GeoJsonFeatureCollection): TalariaFeatureCollection {
  return data as TalariaFeatureCollection;
}

export function ExplorerPage() {
  const { locale, t } = useI18n();
  const [map, setMap] = useState<Map | null>(null);
  const [allEvents, setAllEvents] = useState<TimelineEvent[]>([]);
  const [geojson, setGeojson] = useState<GeoJsonFeatureCollection | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [untilYear, setUntilYear] = useState<number | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [suggestions, setSuggestions] = useState<SearchSuggestion[]>([]);
  const [searchLoading, setSearchLoading] = useState(false);
  const [ingestBusy, setIngestBusy] = useState(false);
  const ingestLock = useRef<string | null>(null);

  const {
    entityId,
    entityLabel,
    personFilter,
    selectedEventId,
    setEntity,
    setPersonFilter,
    setSelectedEventId,
    closeDetail,
  } = useExplorerStore();

  const hasEntity = Boolean(entityId || personFilter);

  useEffect(() => {
    fetchStatus().catch(() => undefined);
  }, []);

  useEffect(() => {
    if (!searchQuery.trim()) {
      setSuggestions([]);
      return;
    }
    let cancelled = false;
    setSearchLoading(true);
    searchEntities(searchQuery, locale)
      .then((items) => {
        if (!cancelled) {
          setSuggestions(collapseToSinglePersonSuggestion(searchQuery, items));
        }
      })
      .catch(() => {
        if (!cancelled) setSuggestions([]);
      })
      .finally(() => {
        if (!cancelled) setSearchLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [searchQuery, locale]);

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
    setError(null);

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
        setError(null);
      } catch (err) {
        if (!cancelled && first) {
          setError(err instanceof Error ? err.message : "load failed");
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
    map.setPadding({ top: 12, bottom: 150, left: 8, right: 8 });
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

  const histogram = useMemo(() => buildYearHistogram(allEvents), [allEvents]);

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

  const runSilentIngest = useCallback(
    async (override: { subject: string; qid?: string | null; entityId?: string | null }) => {
      const subject = override.subject;
      if (!subject) return;
      const lockKey = `explorer:${subject}:${override.qid ?? ""}`;
      if (ingestLock.current === lockKey) return;
      ingestLock.current = lockKey;
      setError(null);
      setIngestBusy(true);
      try {
        const job = await startExplorerIngest({
          subject,
          qid: override.qid,
          live: true,
          wikiLang: locale,
        });
        let boundEntityId = override.entityId;
        const bindEntity = (id?: string | null) => {
          if (!id || boundEntityId === id) return;
          boundEntityId = id;
          setEntity(id, subject, override.qid);
        };
        bindEntity(job.entity_id);
        const result = await pollIngestJob(job.job_id, (tick) => {
          bindEntity(tick.entity_id);
        });
        if (result.status === "failed") {
          setError(result.error ?? t.loadingMap);
          return;
        }
        bindEntity(result.entity_id);
      } catch (err) {
        setError(err instanceof Error ? err.message : t.loadingMap);
      } finally {
        setIngestBusy(false);
        if (ingestLock.current === lockKey) ingestLock.current = null;
      }
    },
    [locale, setEntity, t.loadingMap],
  );

  const handleSelectSuggestion = useCallback(
    (item: SearchSuggestion) => {
      const chosen = preferDenseLocalAlias(item, suggestions);
      setError(null);
      if (chosen.known_locally && chosen.entity_id) {
        setEntity(chosen.entity_id, chosen.label, chosen.qid);
      } else {
        setPersonFilter(chosen.label, chosen.label, chosen.qid);
      }
      void runSilentIngest({
        subject: chosen.label,
        qid: chosen.qid,
        entityId: chosen.entity_id,
      });
    },
    [runSilentIngest, setEntity, setPersonFilter, suggestions],
  );

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

  return (
    <div className="app-shell map-shell flex h-screen flex-col">
      <Navbar
        center={
          <EntitySearchBox
            suggestions={suggestions}
            onSubmitQuery={setSearchQuery}
            onSelect={handleSelectSuggestion}
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
            {t.loadingMap}
          </div>
        ) : null}

        {error ? (
          <p className="absolute top-14 left-1/2 z-10 -translate-x-1/2 rounded-lg bg-red-950/80 px-3 py-1.5 text-sm text-red-200">
            {error}
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
