// web/src/features/istoria/pages/istoria-page.tsx
import { useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { EventDetailCard } from "@/components/detail/event-detail-card";
import { rememberEntity, useExperienceSubject } from "@/components/entity/entity-context";
import { ExperienceLayout } from "@/components/layout/experience-layout";
import { EntitySearchBox } from "@/components/search/entity-search-box";
import { EventMomentSearch } from "@/components/search/event-moment-search";
import { HistoricalTimeline } from "@/components/timeline/historical-timeline";
import { databaseTabs, providerLabel } from "@/features/entity/database-tabs";
import { EntityMap } from "@/features/entity/entity-map";
import { useMediaQuery } from "@/hooks/use-media-query";
import {
  fetchEntityBibliography,
  fetchEventDetail,
  type BibliographyItem,
  type TimelineEvent,
} from "@/lib/api";
import { COMPACT_VIEWPORT_MQ } from "@/lib/breakpoints";
import { getEntityView, type EntityOverview, type Page } from "@/lib/entity-views";
import { useI18n } from "@/lib/i18n";

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
  const { t } = useI18n();
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
  const [events, setEvents] = useState<TimelineEvent[]>([]);
  const [sources, setSources] = useState<BibliographyItem[]>([]);
  const [providers, setProviders] = useState<{ name: string; count: number }[]>([]);
  const [next, setNext] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [selected, setSelected] = useState<TimelineEvent>();
  const [mapFocus, setMapFocus] = useState<{ eventId: string; lat: number; lon: number } | undefined>();
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

  useEffect(() => {
    const controller = new AbortController();
    setEvents([]);
    setSources([]);
    setNext(null);
    setLoading(Boolean(entityId) && (needsTimeline || view === "sources"));
    setError("");
    if (!entityId || (!needsTimeline && view !== "sources")) {
      setLoading(false);
      return;
    }
    const request = async () => {
      if (view === "sources") {
        const result = await fetchEntityBibliography(entityId, {
          limit: 100,
          providers: provider,
          signal: controller.signal,
        });
        if (controller.signal.aborted) return;
        setProviders(result.providers ?? []);
        setSources(result.items);
        setNext(result.next_cursor ?? null);
        return;
      }
      const query = new URLSearchParams(filterString);
      query.set("limit", "200");
      query.set("resolution", "detail");
      const collected: TimelineEvent[] = [];
      do {
        const result = await getEntityView<Page>(entityId, "timeline", query, controller.signal);
        if (controller.signal.aborted) return;
        collected.push(...result.events);
        setEvents([...collected]);
        setNext(result.pagination.next_cursor);
        if (!result.pagination.next_cursor) break;
        query.set("cursor", result.pagination.next_cursor);
      } while (!controller.signal.aborted);
    };
    void request()
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) setError(String(reason));
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [entityId, view, filterString, provider, revision, needsTimeline]);

  useEffect(() => {
    let cancelled = false;
    setSelected(undefined);
    if (!eventId) return;
    void fetchEventDetail(eventId)
      .then((result) => {
        if (!cancelled && result.event?.entity_id === entityId) setSelected(result.event);
      })
      .catch((reason: unknown) => {
        if (!cancelled) setError(String(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [eventId, entityId]);

  const select = (id?: string) => {
    const copy = new URLSearchParams(params);
    if (id) copy.set("event", id);
    else copy.delete("event");
    setParams(copy);
  };
  const update = (key: string, value: string) => {
    const copy = new URLSearchParams(params);
    if (value) copy.set(key, value);
    else copy.delete(key);
    copy.delete("event");
    setParams(copy);
  };
  const focusMoment = (event: TimelineEvent) => {
    select(event.id);
    if (view === "map") {
      const coords = event.coordinates;
      if (coords && Number.isFinite(coords.lat) && Number.isFinite(coords.lon)) {
        setMapFocus({ eventId: event.id, lat: coords.lat, lon: coords.lon });
      } else {
        setMapFocus(undefined);
      }
      return;
    }
    if (view === "timeline") {
      setTimelineZoomEventId(event.id);
      setTimelineZoomToken((token) => token + 1);
    }
  };
  const loadMore = async () => {
    if (!next || loading) return;
    setLoading(true);
    const requestKey = activeRequest.current;
    try {
      if (view === "sources") {
        const result = await fetchEntityBibliography(entityId, {
          limit: 100,
          providers: provider,
          cursor: next,
        });
        if (requestKey !== activeRequest.current) return;
        setSources((old) => [...old, ...result.items]);
        setNext(result.next_cursor ?? null);
      } else {
        const query = new URLSearchParams(filterString);
        query.set("cursor", next);
        query.set("resolution", "detail");
        const result = await getEntityView<Page>(entityId, "timeline", query);
        if (requestKey !== activeRequest.current) return;
        setEvents((old) => [...old, ...result.events]);
        setNext(result.pagination.next_cursor);
      }
    } catch (reason) {
      if (requestKey === activeRequest.current) setError(String(reason));
    } finally {
      if (requestKey === activeRequest.current) setLoading(false);
    }
  };

  const label = overview?.entity.label;
  const pageError = error || (overviewQuery.error ? String(overviewQuery.error) : "");

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
          <EntityMap
            id={entityId}
            filters={filterString}
            selected={eventId ?? undefined}
            focus={mapFocus}
            onSelect={select}
            mode={mobileStage ? "fullscreen" : "desktop"}
          />
        )}
        {view === "timeline" && overview && (
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
        )}
        {view === "timeline" && next && (
          <button className="v3-load-more" type="button" disabled={loading} onClick={() => void loadMore()}>
            Load more moments
          </button>
        )}
      </div>
      {view === "sources" && (
        <section>
          <h2>Source documents</h2>
          <p className="v3-note">
            Catalog matches are not proof of an event. Open an event to inspect its supporting evidence.
          </p>
          <div className="v3-provider-tabs" role="group" aria-label="Filter by database">
            {[{ name: "", count: providers.reduce((sum, item) => sum + item.count, 0) }, ...databaseTabs(providers)].map((item) => (
              <button key={item.name || "all"} aria-pressed={provider === item.name} onClick={() => update("provider", item.name)}>
                {item.name ? providerLabel(item.name) : "All databases"} <span>{item.count}</span>
              </button>
            ))}
          </div>
          <div className="v3-sources">
            {sources.map((source) => (
              <article key={source.id}>
                <span>
                  {providerLabel(source.source_kind)} · {source.document_type.replaceAll("_", " ")}
                </span>
                <h3>{source.title}</h3>
                <p>{source.language?.toUpperCase() ?? ""} {source.academic_status.replaceAll("_", " ")}</p>
                {source.canonical_url && (
                  <a href={source.canonical_url} target="_blank" rel="noreferrer">Open source ↗</a>
                )}
              </article>
            ))}
          </div>
          {!sources.length && !loading && (
            <p>No bibliographic documents indexed for this database and person yet. Event evidence remains available in each event’s details.</p>
          )}
          {next && (
            <button disabled={loading} onClick={() => void loadMore()}>Load more sources</button>
          )}
        </section>
      )}
      {loading && <p role="status">Loading…</p>}
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
