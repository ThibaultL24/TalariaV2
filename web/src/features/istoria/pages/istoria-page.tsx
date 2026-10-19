// web/src/features/istoria/pages/istoria-page.tsx
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { EventDetailCard } from "@/components/detail/event-detail-card";
import { rememberEntity, useExperienceSubject } from "@/components/entity/entity-context";
import { ExperienceLayout } from "@/components/layout/experience-layout";
import { EntitySearchBox } from "@/components/search/entity-search-box";
import { EventMomentSearch } from "@/components/search/event-moment-search";
import { databaseTabs, providerLabel } from "@/features/entity/database-tabs";
import { useMediaQuery } from "@/hooks/use-media-query";
import { useEntityTimeline } from "@/hooks/use-entity-timeline";
import {
  fetchEntityBibliography,
  fetchEventDetail,
  type BibliographyItem,
  type TimelineEvent,
} from "@/lib/api";
import { COMPACT_VIEWPORT_MQ } from "@/lib/breakpoints";
import { getEntityView, type EntityOverview, type Page } from "@/lib/entity-views";
import { useI18n } from "@/lib/i18n";

const EntityMap = lazy(() => import("@/features/entity/entity-map").then((mod) => ({ default: mod.EntityMap })));
const HistoricalTimeline = lazy(() => import("@/components/timeline/historical-timeline").then((mod) => ({ default: mod.HistoricalTimeline })));

function timelineFilters(params: URLSearchParams, view: string): string {
  const filters = new URLSearchParams();
  for (const key of ["from", "to", "types"]) {
    const value = params.get(key);
    if (value && !(view === "timeline" && (key === "from" || key === "to"))) filters.set(key, value);
  }
  return filters.toString();
}

export function IstoriaPage() {
  const {
    entityId,
    view,
    params,
    setParams,
    picker,
    revision,
    pendingSelection,
    openPerson,
  } = useExperienceSubject("istoria");
  const { t, locale } = useI18n();
  const overviewQuery = useQuery({
    queryKey: ["entity", entityId, "overview", revision],
    enabled: Boolean(entityId),
    retry: false,
    staleTime: 30_000,
    refetchOnWindowFocus: false,
    queryFn: ({ signal }) => getEntityView<EntityOverview>(entityId, "overview", new URLSearchParams(), signal),
  });
  const overview = overviewQuery.data;
  useEffect(() => {
    if (!entityId) return;
    rememberEntity(entityId, overview?.entity.label, overview?.entity.qid);
  }, [entityId, overview]);

  const compactViewport = useMediaQuery(COMPACT_VIEWPORT_MQ);
  const immersive = Boolean(entityId) && (view === "timeline" || view === "map");
  const mobileStage = immersive && compactViewport;
  const [toolsOpen, setToolsOpen] = useState(false);
  const [sources, setSources] = useState<BibliographyItem[]>([]);
  const [providers, setProviders] = useState<{ name: string; count: number }[]>([]);
  const [sourceNext, setSourceNext] = useState<string | null>(null);
  const [sourceLoading, setSourceLoading] = useState(false);
  const [sourceError, setSourceError] = useState("");
  const [timelineZoomToken, setTimelineZoomToken] = useState(0);
  const [timelineZoomEventId, setTimelineZoomEventId] = useState<string | undefined>();
  const provider = params.get("provider") ?? "";
  const filterString = timelineFilters(params, view);
  const activeRequest = useRef("");
  activeRequest.current = `${entityId}/${view}?${filterString}&provider=${provider}`;
  const eventId = params.get("event");
  const needsTimeline = view === "timeline" || view === "map";
  const from = Number(params.get("from") ?? overview?.time_bounds.from ?? 0);
  const to = Number(params.get("to") ?? overview?.time_bounds.to ?? from + 1);

  const timeline = useEntityTimeline(entityId, filterString, locale, needsTimeline, revision);
  const events = timeline.events;
  const next = view === "sources" ? sourceNext : (timeline.hasMore ? "next" : null);
  const loading = view === "sources" ? sourceLoading : (needsTimeline && timeline.loading);
  const eventDetailQuery = useQuery({
    queryKey: ["istoria", "event", entityId, eventId, locale],
    enabled: Boolean(entityId && eventId),
    retry: false,
    staleTime: 60_000,
    refetchOnWindowFocus: false,
    queryFn: ({ signal }) => fetchEventDetail(eventId!, locale, signal),
  });
  const selected = eventDetailQuery.data?.event?.entity_id === entityId
    ? eventDetailQuery.data.event
    : undefined;

  // The URL event ID is the shared selection across timeline, map and details.
  // The detail API can resolve deep links absent from the current timeline page.
  const mapFocus = useMemo(() => {
    if (view !== "map" || !eventId) return undefined;
    const coords = selected?.coordinates ?? events.find((item) => item.id === eventId)?.coordinates;
    if (!coords || !Number.isFinite(coords.lat) || !Number.isFinite(coords.lon)) return undefined;
    return { eventId, lat: coords.lat, lon: coords.lon };
  }, [view, eventId, selected, events]);

  useEffect(() => {
    if (!entityId || view !== "sources") return;
    const controller = new AbortController();
    setSources([]);
    setProviders([]);
    setSourceNext(null);
    setSourceLoading(true);
    setSourceError("");
    void fetchEntityBibliography(entityId, {
      limit: 100,
      providers: provider,
      signal: controller.signal,
    })
      .then((result) => {
        if (controller.signal.aborted) return;
        setProviders(result.providers ?? []);
        setSources(result.items);
        setSourceNext(result.next_cursor ?? null);
      })
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) setSourceError(String(reason));
      })
      .finally(() => {
        if (!controller.signal.aborted) setSourceLoading(false);
      });
    return () => controller.abort();
  }, [entityId, view, provider, revision]);

  const select = useCallback((id?: string) => {
    const copy = new URLSearchParams(params);
    if (id) copy.set("event", id);
    else copy.delete("event");
    setParams(copy);
  }, [params, setParams]);

  const update = (key: string, value: string) => {
    const copy = new URLSearchParams(params);
    if (value) copy.set(key, value);
    else copy.delete(key);
    // Preserve the selected event when navigating or changing the visible period.
    setParams(copy);
  };

  const focusMoment = (event: TimelineEvent) => {
    select(event.id);
    if (view === "timeline") {
      setTimelineZoomEventId(event.id);
      setTimelineZoomToken((token) => token + 1);
    }
  };

  const loadMore = async () => {
    if (loading) return;
    if (view !== "sources") {
      if (timeline.hasMore) await timeline.loadMore();
      return;
    }
    if (!sourceNext) return;
    setSourceLoading(true);
    const requestKey = activeRequest.current;
    try {
      const result = await fetchEntityBibliography(entityId, {
        limit: 100,
        providers: provider,
        cursor: sourceNext,
      });
      if (requestKey !== activeRequest.current) return;
      setSources((old) => [...old, ...result.items]);
      setSourceNext(result.next_cursor ?? null);
    } catch (reason) {
      if (requestKey === activeRequest.current) setSourceError(String(reason));
    } finally {
      if (requestKey === activeRequest.current) setSourceLoading(false);
    }
  };

  const label = overview?.entity.label;
  const pageError = sourceError
    || (timeline.error && needsTimeline ? String(timeline.error) : "")
    || (eventDetailQuery.error ? String(eventDetailQuery.error) : "")
    || (overviewQuery.error ? String(overviewQuery.error) : "");

  return (
    <ExperienceLayout
      experience="istoria"
      entityId={entityId}
      label={label}
      immersive={immersive}
      mobileStage={mobileStage}
      toolsOpen={toolsOpen}
      onToggleTools={() => setToolsOpen((open) => !open)}
      showHeroBand={!entityId || view === "overview"}
      search={
        <>
          <EntitySearchBox
            suggestions={picker.suggestions}
            onSubmitQuery={picker.setSearchQuery}
            isLoading={picker.searchLoading}
            onSelect={(item) => {
              pendingSelection.current = true;
              const selectedId = picker.selectPerson(item);
              if (selectedId) openPerson(selectedId);
            }}
          />
          {picker.ingestBusy && (
            <p role="status">
              Collecting sources · {picker.timelineEvents} events · {picker.mapPins} map points
            </p>
          )}
          {picker.error && <p role="alert">{picker.error}</p>}
          {!entityId && !picker.ingestBusy && (
            <p className="v3-note">Rechercher une personnalité pour découvrir sa vie et ses sources.</p>
          )}
        </>
      }
    >
      {pageError && <p className="v3-inline-alert" role="alert">{pageError}</p>}
      {view === "overview" && overview && (
        <section className="v3-stats">
          {Object.entries(overview.stats).map(([key, value]) => (
            <article key={key}>
              <strong>{value.toLocaleString()}</strong>
              <span>{key.replaceAll("_", " ")}</span>
            </article>
          ))}
        </section>
      )}
      <div className={immersive ? "v3-explorer-stage" : undefined}>
        {entityId && needsTimeline && (
          <>
            {mobileStage && toolsOpen ? (
              <button
                type="button"
                className="v3-mobile-drawer-scrim"
                aria-label={t.close}
                onClick={() => setToolsOpen(false)}
              />
            ) : null}
            {(!mobileStage || toolsOpen) && (
              <div className={`v3-moment-search-row${mobileStage && toolsOpen ? " is-open" : ""}`}>
                <EventMomentSearch events={events} loading={loading} onPick={focusMoment} />
                {view === "map" && (
                  <form className="v3-filters v3-filters--compact" onSubmit={(event) => event.preventDefault()}>
                    <label>
                      From{" "}
                      <input
                        type="number"
                        value={params.get("from") ?? ""}
                        onChange={(event) => update("from", event.target.value)}
                      />
                    </label>
                    <label>
                      To{" "}
                      <input
                        type="number"
                        value={params.get("to") ?? ""}
                        onChange={(event) => update("to", event.target.value)}
                      />
                    </label>
                  </form>
                )}
              </div>
            )}
          </>
        )}
        {view === "map" && entityId && (
          <Suspense fallback={<p role="status">{t.loading}</p>}>
          <EntityMap
            id={entityId}
            filters={filterString}
            selected={eventId ?? undefined}
            focus={mapFocus}
            onSelect={select}
            mode={mobileStage ? "fullscreen" : "desktop"}
          />
          </Suspense>
        )}
        {view === "timeline" && overview && (
          <Suspense fallback={<p role="status">{t.loading}</p>}>
          <>
            <p className="v3-note">
              {t.visiblePeriod(String(from || "?"), String(to || "?"))}
            </p>
            <HistoricalTimeline
            key={entityId}
            events={events}
            bounds={[overview.time_bounds.from ?? from, overview.time_bounds.to ?? to]}
            from={from}
            to={to}
            spotlightEventId={eventId ?? undefined}
            zoomToEventId={timelineZoomEventId}
            zoomToEventToken={timelineZoomToken}
            mode={mobileStage ? "fullscreen" : "desktop"}
            onSelect={select}
            onZoom={(start, end) => {
              const copy = new URLSearchParams(params);
              copy.set("from", String(start));
              copy.set("to", String(end));
              setParams(copy, { replace: true });
            }}
            />
          </>
          </Suspense>
        )}
        {view === "timeline" && next && (
          <button className="v3-load-more" type="button" disabled={loading} onClick={() => void loadMore()}>
            {t.loadMoreMoments}
          </button>
        )}
      </div>
      {view === "sources" && (
        <section>
          <h2>{t.sourceDocuments}</h2>
          <p className="v3-note">{t.sourceDocumentsNote}</p>
          <div className="v3-provider-tabs" role="group" aria-label={t.filterByPlaceKind}>
            {[{ name: "", count: providers.reduce((sum, item) => sum + item.count, 0) }, ...databaseTabs(providers)]
              .map((item) => (
              <button key={item.name || "all"} aria-pressed={provider === item.name} onClick={() => update("provider", item.name)}>
                {item.name ? providerLabel(item.name) : t.allDatabases} <span>{item.count}</span>
              </button>
            ))}
          </div>
          <div className="v3-sources">
            {[...sources].sort((left, right) => Number(right.language?.toLowerCase().startsWith(locale)) - Number(left.language?.toLowerCase().startsWith(locale))).map((source) => (
              <article key={source.id}>
                <span>
                  {providerLabel(source.source_kind)} · {source.document_type.replaceAll("_", " ")}
                </span>
                <h3>{source.title}</h3>
                <p>{source.language?.toUpperCase() ?? ""} {source.academic_status.replaceAll("_", " ")}</p>
                {source.canonical_url && (
                  <a href={source.canonical_url} target="_blank" rel="noreferrer">{t.openSource} ↗</a>
                )}
              </article>
            ))}
          </div>
          {!sources.length && !loading && (
            <p>{t.bibliographyEmpty}</p>
          )}
          {next && (
            <button disabled={loading} onClick={() => void loadMore()}>{t.loadMoreSources}</button>
          )}
        </section>
      )}
      {loading && <p role="status">{t.loading}</p>}
      {selected && (
        <aside className="v3-detail">
          <div className="v3-detail-links">
            {selected.map_eligible && (
              <Link to={`/istoria/${entityId}/map?${params}`}>View on map</Link>
            )}
            <Link to={`/istoria/${entityId}/timeline?${params}`}>View in timeline</Link>
          </div>
          <EventDetailCard event={selected} onClose={() => select()} />
        </aside>
      )}
    </ExperienceLayout>
  );
}
