// web/src/pages/explorer-page.tsx
import { useCallback, useEffect, useMemo, useState } from "react";
import type { Map } from "maplibre-gl";
import { ExplorerEventFilters } from "@/components/filters/explorer-event-filters";
import { EntityProfile } from "@/components/explorer/entity-profile";
import { Navbar } from "@/components/layout/navbar";
import { EventDetailCard } from "@/components/detail/event-detail-card";
import { ExplorerMapTimelineBar } from "@/components/map/explorer-map-timeline-bar";
import { MapCanvas } from "@/components/map/map-canvas";
import { MapInteractions } from "@/components/map/map-interactions";
import { MapLayers } from "@/components/map/map-layers";
import { MapSourceManager } from "@/components/map/map-source-manager";
import { EntitySearchBox } from "@/components/search/entity-search-box";
import { TimelineList } from "@/components/timeline/timeline-list";
import { mapTimelineEventToItem } from "@/features/events/mappers/timeline";
import {
  fetchGeoJson,
  fetchStatus,
  fetchTimeline,
  searchEntities,
  type GeoJsonFeatureCollection,
  type StatusResponse,
  type TimelineEvent,
} from "@/lib/api";
import type { SearchSuggestion } from "@/lib/schemas/entity";
import {
  buildYearBounds,
  buildYearHistogram,
  filterGeoJsonByTaxonomy,
  filterGeoJsonByYearRange,
  filterTimelineByTaxonomy,
  filterTimelineByYearRange,
} from "@/lib/geo";
import { useExplorerStore } from "@/stores/explorer-store";

export function ExplorerPage() {
  const [map, setMap] = useState<Map | null>(null);
  const [allEvents, setAllEvents] = useState<TimelineEvent[]>([]);
  const [geojson, setGeojson] = useState<GeoJsonFeatureCollection | null>(null);
  const [status, setStatus] = useState<StatusResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [yearRange, setYearRange] = useState<{ min: number; max: number } | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [suggestions, setSuggestions] = useState<SearchSuggestion[]>([]);
  const [searchLoading, setSearchLoading] = useState(false);

  const {
    entityId,
    entityLabel,
    personFilter,
    selectedEventId,
    filters,
    setEntity,
    setPersonFilter,
    setSelectedEventId,
    toggleTypeFilter,
    toggleStatusFilter,
    clearFilters,
    closeDetail,
  } = useExplorerStore();

  const hasEntity = Boolean(entityId || personFilter);

  useEffect(() => {
    fetchStatus()
      .then(setStatus)
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    if (!searchQuery.trim()) {
      setSuggestions([]);
      return;
    }

    let cancelled = false;
    setSearchLoading(true);

    searchEntities(searchQuery)
      .then((items) => {
        if (!cancelled) setSuggestions(items);
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
  }, [searchQuery]);

  useEffect(() => {
    if (!hasEntity) {
      setAllEvents([]);
      setGeojson(null);
      setYearRange(null);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);

    const query = {
      entityId: entityId ?? undefined,
      person: personFilter ?? undefined,
    };

    Promise.all([fetchTimeline(query), fetchGeoJson(query)])
      .then(([timeline, mapData]) => {
        if (cancelled) return;
        setAllEvents(timeline.events);
        setGeojson(mapData);
        setYearRange(buildYearBounds(timeline.events));
      })
      .catch((err) => {
        if (!cancelled) setError(err instanceof Error ? err.message : "load failed");
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [entityId, personFilter, hasEntity]);

  const dataBounds = useMemo(() => buildYearBounds(allEvents), [allEvents]);
  const activeRange = yearRange ?? dataBounds;

  const taxonomyEvents = useMemo(
    () => filterTimelineByTaxonomy(allEvents, filters.types, filters.statuses),
    [allEvents, filters.types, filters.statuses],
  );

  const filteredEvents = useMemo(
    () => filterTimelineByYearRange(taxonomyEvents, activeRange),
    [taxonomyEvents, activeRange],
  );

  const filteredGeoJson = useMemo(() => {
    if (!geojson) return { type: "FeatureCollection" as const, features: [] };
    const byTaxonomy = filterGeoJsonByTaxonomy(geojson, filters.types, filters.statuses);
    return filterGeoJsonByYearRange(byTaxonomy, activeRange);
  }, [geojson, filters.types, filters.statuses, activeRange]);

  const histogram = useMemo(() => buildYearHistogram(taxonomyEvents), [taxonomyEvents]);

  const availableTypes = useMemo(
    () => [...new Set(allEvents.map((event) => event.event_type))].sort(),
    [allEvents],
  );

  const availableStatuses = useMemo(
    () => [...new Set(allEvents.map((event) => event.epistemic_status))].sort(),
    [allEvents],
  );

  const timelineItems = useMemo(
    () => filteredEvents.map((event) => mapTimelineEventToItem(event)),
    [filteredEvents],
  );

  const selectedEvent = useMemo(
    () => allEvents.find((event) => event.id === selectedEventId) ?? null,
    [allEvents, selectedEventId],
  );

  const handleSelectSuggestion = useCallback(
    (item: SearchSuggestion) => {
      if (item.known_locally && item.entity_id) {
        setEntity(item.entity_id, item.label);
        return;
      }
      setPersonFilter(item.label, item.label);
    },
    [setEntity, setPersonFilter],
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

  return (
    <div className="app-shell map-shell flex h-screen flex-col">
      <Navbar status={status} />

      <div className="map-shell__body relative flex min-h-0 flex-1 flex-col overflow-hidden md:flex-row">
        <aside className="map-shell__sidebar flex max-h-[min(52vh,28rem)] w-full shrink-0 flex-col border-b border-(--color-border-subtle) md:max-h-none md:w-[min(100%,22rem)] md:min-w-[16rem] md:max-w-[22rem] md:border-b-0 md:border-r lg:w-80">
          <div className="border-b border-(--color-border-subtle) p-3">
            <EntitySearchBox
              suggestions={suggestions}
              onSubmitQuery={setSearchQuery}
              onSelect={handleSelectSuggestion}
              isLoading={searchLoading}
            />
          </div>

          {entityLabel ? (
            <EntityProfile
              name={entityLabel}
              eventCount={allEvents.length}
            />
          ) : null}

          {hasEntity && allEvents.length > 0 ? (
            <ExplorerEventFilters
              availableTypes={availableTypes}
              availableStatuses={availableStatuses}
              selectedTypes={filters.types}
              selectedStatuses={filters.statuses}
              onToggleType={toggleTypeFilter}
              onToggleStatus={toggleStatusFilter}
              onClear={clearFilters}
            />
          ) : null}

          <div className="min-h-0 flex-1 overflow-y-auto">
            {error ? <p className="p-4 text-sm text-red-400">{error}</p> : null}
            <TimelineList
              items={timelineItems}
              hasEntity={hasEntity}
              isLoading={loading}
              onSelectEvent={handleSelectEvent}
            />
          </div>
        </aside>

        <main className="map-shell__map-stage relative min-h-[min(40vh,24rem)] min-w-0 flex-1 md:min-h-0">
          <div className="absolute inset-0 min-h-0">
            <MapCanvas onReady={setMap} />
          </div>
          <MapSourceManager map={map} data={hasEntity ? filteredGeoJson : undefined} />
          <MapLayers
            map={map}
            data={hasEntity ? filteredGeoJson : undefined}
            selectedEventId={selectedEventId}
          />
          <MapInteractions map={map} onSelectEvent={handleSelectEvent} />

          {!loading && hasEntity && allEvents.length > 0 && dataBounds ? (
            <ExplorerMapTimelineBar
              bounds={dataBounds}
              range={activeRange}
              onRangeChange={setYearRange}
              visibleCount={filteredEvents.length}
              totalCount={taxonomyEvents.length}
              yearHistogram={histogram}
            />
          ) : null}

          {!hasEntity ? (
            <div className="surface-nav pointer-events-none absolute bottom-3 left-3 z-10 max-w-sm rounded-lg border border-(--map-panel-border) px-3 py-2 text-xs text-(--color-text-secondary)">
              Search for a person to load events on the map.
            </div>
          ) : null}

          {selectedEventId && selectedEvent ? (
            <div
              className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-6"
              role="presentation"
            >
              <button
                type="button"
                className="absolute inset-0 bg-black/45 backdrop-blur-sm"
                aria-label="Close detail"
                onClick={closeDetail}
              />
              <div
                className="nebula-event-detail relative flex max-h-[min(90vh,820px)] w-full max-w-lg flex-col overflow-hidden rounded-xl"
                role="dialog"
                aria-modal="true"
                aria-labelledby="event-detail-card-title"
              >
                <EventDetailCard event={selectedEvent} onClose={closeDetail} />
              </div>
            </div>
          ) : null}
        </main>
      </div>
    </div>
  );
}
