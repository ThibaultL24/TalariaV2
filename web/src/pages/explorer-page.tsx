// web/src/pages/explorer-page.tsx
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Map } from "maplibre-gl";
import { ExplorerEventFilters } from "@/components/filters/explorer-event-filters";
import { EntityProfile } from "@/components/explorer/entity-profile";
import { AgoraPanel } from "@/components/explorer/agora-panel";
import { LaneIngestBar } from "@/components/explorer/lane-ingest-bar";
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
  fetchEntity,
  fetchEntityBibliography,
  fetchEntityClaims,
  fetchGeoJson,
  fetchIngestJob,
  fetchPeriods,
  fetchProfiles,
  fetchStatus,
  fetchTimeline,
  searchEntities,
  startAgoraIngest,
  startExplorerIngest,
  type BibliographyItem,
  type EntityClaim,
  type GeoJsonFeatureCollection,
  type StatusResponse,
  type TimelineEvent,
} from "@/lib/api";
import type { PeriodFacet, ProfileFacet, SearchSuggestion } from "@/lib/schemas/entity";
import {
  buildYearBounds,
  buildYearHistogram,
  filterGeoJsonByTaxonomy,
  filterGeoJsonByYearRange,
  filterTimelineByTaxonomy,
  filterTimelineByYearRange,
} from "@/lib/geo";
import { strings } from "@/lib/strings";
import { useExplorerStore } from "@/stores/explorer-store";

const POLL_MS = 5000;
const LIVE_INGEST_POLL_MS = 1500;
const INGEST_POLL_MS = 1500;
const INGEST_TIMEOUT_MS = 45 * 60 * 1000;
const LIVE_LIMIT = 2000;

async function pollIngestJob(
  jobId: string,
  onTick: (job: Awaited<ReturnType<typeof fetchIngestJob>>) => void,
): Promise<Awaited<ReturnType<typeof fetchIngestJob>>> {
  const deadline = Date.now() + INGEST_TIMEOUT_MS;
  while (Date.now() < deadline) {
    const job = await fetchIngestJob(jobId);
    onTick(job);
    if (job.status !== "running" && job.status !== "queued") {
      return job;
    }
    await new Promise((resolve) => setTimeout(resolve, INGEST_POLL_MS));
  }
  throw new Error("timeout");
}

function namesOverlap(left: string, right: string): boolean {
  const a = left.trim().toLowerCase();
  const b = right.trim().toLowerCase();
  return a.includes(b) || b.includes(a);
}

/** Prefer the dense local entity when aliases split (Napoleon vs Napoleon Bonaparte). */
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
  const [periods, setPeriods] = useState<PeriodFacet[]>([]);
  const [profiles, setProfiles] = useState<ProfileFacet[]>([]);
  const [entityProfiles, setEntityProfiles] = useState<Array<{ slug: string; label: string }>>([]);
  const [sidebarTab, setSidebarTab] = useState<"timeline" | "agora">("timeline");
  const [agoraClaims, setAgoraClaims] = useState<EntityClaim[]>([]);
  const [bibliography, setBibliography] = useState<BibliographyItem[]>([]);
  const [agoraLoading, setAgoraLoading] = useState(false);
  const [bibliographyLoading, setBibliographyLoading] = useState(false);
  const [ingestStatus, setIngestStatus] = useState<string | null>(null);
  const rangeTouched = useRef(false);
  const ingestLock = useRef<string | null>(null);

  const {
    entityId,
    entityLabel,
    entityQid,
    personFilter,
    selectedEventId,
    filters,
    setEntity,
    setPersonFilter,
    setEntityQid,
    setSelectedEventId,
    toggleTypeFilter,
    toggleStatusFilter,
    setProfileFilter,
    setPeriodFilter,
    clearFilters,
    closeDetail,
  } = useExplorerStore();

  const hasEntity = Boolean(entityId || personFilter);
  const ingestLive =
    Boolean(ingestStatus) &&
    ingestStatus !== strings.ingestDone &&
    ingestStatus !== strings.agoraDone;

  useEffect(() => {
    let cancelled = false;
    function loadStatus() {
      fetchStatus()
        .then((row) => {
          if (!cancelled) setStatus(row);
        })
        .catch(() => undefined);
    }
    loadStatus();
    const tick = window.setInterval(loadStatus, POLL_MS);
    Promise.all([fetchPeriods(), fetchProfiles()])
      .then(([periodRows, profileRows]) => {
        if (cancelled) return;
        setPeriods(periodRows);
        setProfiles(profileRows);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
      window.clearInterval(tick);
    };
  }, []);

  useEffect(() => {
    if (!entityId) {
      setEntityProfiles([]);
      return;
    }
    let cancelled = false;
    fetchEntity(entityId)
      .then((entity) => {
        if (cancelled || !entity) return;
        if (entity.qid) setEntityQid(entity.qid);
        setEntityProfiles(
          (entity.profiles ?? []).map((profile) => ({ slug: profile.slug, label: profile.label })),
        );
      })
      .catch(() => {
        if (!cancelled) setEntityProfiles([]);
      });
    return () => {
      cancelled = true;
    };
  }, [entityId, setEntityQid]);

  useEffect(() => {
    rangeTouched.current = false;
    if (!entityId) {
      setAgoraClaims([]);
      setBibliography([]);
      return;
    }
    const id = entityId;
    let cancelled = false;
    setAgoraLoading(true);
    setBibliographyLoading(true);
    async function loadAgora() {
      try {
        const [claims, bib] = await Promise.all([
          fetchEntityClaims(id),
          fetchEntityBibliography(id, { limit: 40 }),
        ]);
        if (!cancelled) {
          setAgoraClaims(claims);
          setBibliography(bib.items ?? []);
        }
      } catch {
        if (!cancelled) {
          setAgoraClaims([]);
          setBibliography([]);
        }
      } finally {
        if (!cancelled) {
          setAgoraLoading(false);
          setBibliographyLoading(false);
        }
      }
    }
    loadAgora();
    const tick = window.setInterval(loadAgora, POLL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(tick);
    };
  }, [entityId]);

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
      rangeTouched.current = false;
      return;
    }

    let cancelled = false;
    let inFlight = false;
    let first = true;
    setError(null);

    const query = {
      entityId: entityId ?? undefined,
      person: personFilter ?? undefined,
      profileSlug: filters.profileSlug,
      periodSlug: filters.periodSlug,
      limit: LIVE_LIMIT,
    };

    async function load() {
      if (inFlight) return;
      inFlight = true;
      if (first && !ingestLive) setLoading(true);
      try {
        const [timeline, mapData] = await Promise.all([
          fetchTimeline(query),
          fetchGeoJson(query),
        ]);
        if (cancelled) return;
        setAllEvents(timeline.events);
        setGeojson(mapData);
        const bounds = buildYearBounds(timeline.events);
        setYearRange((prev) => {
          if (!bounds) return prev;
          if (!prev || !rangeTouched.current) return bounds;
          return prev;
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
    const tick = window.setInterval(load, ingestLive ? LIVE_INGEST_POLL_MS : POLL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(tick);
    };
  }, [entityId, personFilter, hasEntity, ingestLive, filters.profileSlug, filters.periodSlug]);

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

  const handleYearRange = useCallback((range: { min: number; max: number }) => {
    rangeTouched.current = true;
    setYearRange(range);
  }, []);

  const runLaneIngest = useCallback(
    async (
      lane: "explorer" | "agora",
      override?: { subject: string; qid?: string | null; entityId?: string | null },
    ) => {
      const subject = override?.subject ?? entityLabel ?? personFilter;
      if (!subject) return;
      const qid = override?.qid ?? entityQid;
      const startingEntityId = override?.entityId ?? entityId;
      const lockKey = `${lane}:${subject}:${qid ?? ""}`;
      if (ingestLock.current === lockKey) return;
      ingestLock.current = lockKey;
      setError(null);
      try {
        setIngestStatus(lane === "explorer" ? strings.ingestQueued : strings.agoraQueued);
        const job =
          lane === "explorer"
            ? await startExplorerIngest({
                subject,
                qid,
                live: true,
              })
            : await startAgoraIngest({
                subject,
                qid,
                live: true,
              });
        let boundEntityId = startingEntityId;
        const bindEntity = (id?: string | null) => {
          if (!id || boundEntityId === id) return;
          boundEntityId = id;
          setEntity(id, subject, qid);
        };
        bindEntity(job.entity_id);
        const running =
          lane === "explorer" ? strings.ingestRunning : strings.agoraRunning;
        const result = await pollIngestJob(job.job_id, (tick) => {
          bindEntity(tick.entity_id);
          if (lane === "explorer") {
            setIngestStatus(
              strings.ingestRunningCounts(tick.timeline_events ?? 0, tick.map_events ?? 0),
            );
          } else {
            setIngestStatus(running);
          }
        });
        if (result.status === "failed") {
          setIngestStatus(null);
          setError(`${strings.ingestFailed}: ${result.error ?? "unknown"}`);
          return;
        }
        bindEntity(result.entity_id);
        setIngestStatus(lane === "explorer" ? strings.ingestDone : strings.agoraDone);
        window.setTimeout(() => setIngestStatus(null), 2500);
      } catch (err) {
        setIngestStatus(null);
        setError(err instanceof Error ? err.message : strings.ingestFailed);
      } finally {
        if (ingestLock.current === lockKey) ingestLock.current = null;
      }
    },
    [entityId, entityLabel, entityQid, personFilter, setEntity],
  );

  const handleSelectSuggestion = useCallback(
    (item: SearchSuggestion) => {
      const chosen = preferDenseLocalAlias(item, suggestions);
      setIngestStatus(null);
      setError(null);
      if (chosen.known_locally && chosen.entity_id) {
        setEntity(chosen.entity_id, chosen.label, chosen.qid);
      } else {
        setPersonFilter(chosen.label, chosen.label, chosen.qid);
      }
      void runLaneIngest("explorer", {
        subject: chosen.label,
        qid: chosen.qid,
        entityId: chosen.entity_id,
      });
    },
    [runLaneIngest, setEntity, setPersonFilter, suggestions],
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
              onSelect={(item) => {
                handleSelectSuggestion(item);
              }}
              isLoading={searchLoading}
            />
          </div>

          {entityLabel ? (
            <EntityProfile
              name={entityLabel}
              qid={entityQid}
              eventCount={allEvents.length}
              mapCount={geojson?.features.length ?? 0}
              agoraCount={agoraClaims.length}
              bibliographyCount={bibliography.length}
              profiles={entityProfiles}
            />
          ) : null}

          {hasEntity ? (
            <ExplorerEventFilters
              availableTypes={availableTypes}
              availableStatuses={availableStatuses}
              selectedTypes={filters.types}
              selectedStatuses={filters.statuses}
              profiles={
                entityProfiles.length > 0
                  ? entityProfiles.map((profile) => ({
                      slug: profile.slug,
                      label: profile.label,
                      entity_count: 1,
                    }))
                  : profiles
              }
              periods={periods.filter((period) => period.kind === "century" || period.kind === "era")}
              selectedProfileSlug={filters.profileSlug}
              selectedPeriodSlug={filters.periodSlug}
              onToggleType={toggleTypeFilter}
              onToggleStatus={toggleStatusFilter}
              onToggleProfile={setProfileFilter}
              onTogglePeriod={setPeriodFilter}
              onClear={clearFilters}
            />
          ) : null}

          {hasEntity ? (
            <div className="flex border-b border-(--color-border-subtle)">
              <button
                type="button"
                className={`flex-1 px-3 py-2 text-[11px] font-semibold uppercase tracking-wide ${
                  sidebarTab === "timeline"
                    ? "text-(--color-text-primary)"
                    : "text-(--color-text-muted)"
                }`}
                onClick={() => setSidebarTab("timeline")}
                title={strings.laneExplorerHint}
              >
                {strings.laneExplorerTitle}
              </button>
              <button
                type="button"
                className={`flex-1 px-3 py-2 text-[11px] font-semibold uppercase tracking-wide ${
                  sidebarTab === "agora"
                    ? "text-(--color-text-primary)"
                    : "text-(--color-text-muted)"
                }`}
                onClick={() => setSidebarTab("agora")}
                title={strings.laneAgoraHint}
              >
                {strings.laneAgoraTitle}
                {agoraClaims.length + bibliography.length > 0
                  ? ` (${agoraClaims.length + bibliography.length})`
                  : ""}
              </button>
            </div>
          ) : null}

          {hasEntity ? (
            <p className="border-b border-(--color-border-subtle) px-3 py-2 text-[10px] leading-relaxed text-(--color-text-muted)">
              {sidebarTab === "timeline" ? strings.laneExplorerHint : strings.laneAgoraHint}
            </p>
          ) : null}

          {hasEntity ? (
            <LaneIngestBar
              lane={sidebarTab === "agora" ? "agora" : "explorer"}
              busy={ingestLive}
              status={ingestStatus}
              onRun={() => {
                void runLaneIngest(sidebarTab === "agora" ? "agora" : "explorer");
              }}
            />
          ) : null}

          <div className="min-h-0 flex-1 overflow-y-auto">
            {error ? <p className="p-4 text-sm text-red-400">{error}</p> : null}
            {sidebarTab === "agora" && hasEntity ? (
              <AgoraPanel
                claims={agoraClaims}
                bibliography={bibliography}
                isLoading={agoraLoading}
                bibliographyLoading={bibliographyLoading}
                onOpenEvent={handleSelectEvent}
              />
            ) : (
              <TimelineList
                items={timelineItems}
                hasEntity={hasEntity}
                isLoading={loading}
                onSelectEvent={handleSelectEvent}
              />
            )}
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
              onRangeChange={handleYearRange}
              visibleCount={filteredEvents.length}
              totalCount={taxonomyEvents.length}
              yearHistogram={histogram}
            />
          ) : null}

          {!hasEntity ? (
            <div className="surface-nav pointer-events-none absolute bottom-3 left-3 z-10 max-w-sm rounded-lg border border-(--map-panel-border) px-3 py-2 text-xs text-(--color-text-secondary)">
              {strings.emptySearch}
            </div>
          ) : (
            <div className="pointer-events-none absolute bottom-3 left-3 z-10 max-w-xs rounded-lg border border-(--map-panel-border) bg-(--color-bg-elevated)/80 px-3 py-2 text-[11px] leading-relaxed text-(--color-text-secondary) backdrop-blur-sm">
              {sidebarTab === "agora" ? strings.laneAgoraHint : strings.laneExplorerHint}
            </div>
          )}

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
                className="nebula-event-detail relative flex max-h-[min(90vh,860px)] w-full max-w-xl flex-col overflow-hidden rounded-xl"
                role="dialog"
                aria-modal="true"
                aria-labelledby="event-detail-card-title"
              >
                <EventDetailCard
                  event={selectedEvent}
                  onClose={closeDetail}
                  offlineOnly={Boolean(status?.offline_only)}
                />
              </div>
            </div>
          ) : null}
        </main>
      </div>
    </div>
  );
}
