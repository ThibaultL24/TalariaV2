import { useEffect, useMemo, useRef, useState } from "react";
import { Link, NavLink, useNavigate, useParams, useSearchParams } from "react-router-dom";
import { EntitySearchBox } from "@/components/search/entity-search-box";
import { usePersonPicker } from "@/hooks/use-person-picker";
import { useExplorerStore } from "@/stores/explorer-store";
import { Navbar } from "@/components/layout/navbar";
import { TalariaSandalMark } from "@/components/brand/talaria-sandal-mark";
import { EventDetailCard } from "@/components/detail/event-detail-card";
import {
  fetchEntityBibliography,
  fetchEventDetail,
  type BibliographyItem,
  type TimelineEvent,
} from "@/lib/api";
import {
  fetchVisitHeritage,
  fetchVisitNow,
  getEntityView,
  eventDate,
  type EntityOverview,
  type Page,
  type VisitOpportunity,
} from "@/lib/entity-views";
import { EventMomentSearch } from "@/components/search/event-moment-search";
import { HistoricalTimeline } from "@/components/timeline/historical-timeline";
import { EntityVisitMap } from "@/features/visit/entity-visit-map";
import { VisitHeritageList } from "@/features/visit/visit-heritage-list";
import { VisitNowList } from "@/features/visit/visit-now-list";
import { VisitLensToggle, useExplorerLens } from "@/features/visit/visit-lens-toggle";
import { selectChronologicalPreview } from "@/lib/chronological-preview";
import { eventTypeLabel } from "@/lib/event-taxonomy";
import { useI18n } from "@/lib/i18n";
import { EntityMap } from "./entity-map";
import { useMediaQuery } from "@/hooks/use-media-query";
import { COMPACT_VIEWPORT_MQ } from "@/lib/breakpoints";

export function EntityPage() {
  const { entityId: routeEntityId, view = "overview" } = useParams();
  const [params, setParams] = useSearchParams();
  const navigate = useNavigate();
  const storedId = useExplorerStore(s => s.entityId);
  const pendingPerson = useExplorerStore(s => s.personFilter);
  const entityId = routeEntityId ?? params.get("entity") ?? storedId ?? "";
  const [revision, setRevision] = useState(0);
  const picker = usePersonPicker({ onCountsChanged: () => setRevision(value => value + 1) });
  const pendingSelection = useRef(false);
  const launchedPerson = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (!routeEntityId && !entityId && pendingPerson && !pendingSelection.current && launchedPerson.current !== pendingPerson) {
      launchedPerson.current = pendingPerson;
      pendingSelection.current = true;
      picker.selectPerson({ label: pendingPerson, qid: useExplorerStore.getState().entityQid, known_locally: false });
    }
  }, [routeEntityId, entityId, pendingPerson, picker.selectPerson]);
  useEffect(() => {
    if (storedId && pendingSelection.current) {
      pendingSelection.current = false;
      navigate(`/entities/${storedId}/map`);
    } else if (!routeEntityId && entityId) {
      navigate(`/entities/${entityId}/map`, { replace: true });
    }
  }, [storedId, entityId, routeEntityId, navigate]);
  const [overview, setOverview] = useState<EntityOverview>();
  const [events, setEvents] = useState<TimelineEvent[]>([]);
  const provider = params.get("provider") ?? "";
  const [providers, setProviders] = useState<{name: string; count: number}[]>([]);
  const [sources, setSources] = useState<BibliographyItem[]>([]);
  const [next, setNext] = useState<string | null>(null);
  const [selected, setSelected] = useState<TimelineEvent>();
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [mapFocus, setMapFocus] = useState<
    { eventId: string; lat: number; lon: number } | undefined
  >();
  const [timelineZoomToken, setTimelineZoomToken] = useState(0);
  const [timelineZoomEventId, setTimelineZoomEventId] = useState<string | undefined>();
  const lens = useExplorerLens();
  const compactViewport = useMediaQuery(COMPACT_VIEWPORT_MQ);
  const scholarImmersive = lens === "scholar" && (view === "timeline" || view === "map");
  const mobileStage = scholarImmersive && compactViewport;
  const stageMode = mobileStage ? "fullscreen" : "desktop";
  const [toolsOpen, setToolsOpen] = useState(false);
  const { locale, t } = useI18n();
  const scholarTabs = ["overview", "timeline", "map", "sources"] as const;
  const visitTabs = ["map", "heritage", "now"] as const;
  const [visitNow, setVisitNow] = useState<VisitOpportunity[]>([]);
  const tabs = lens === "visit" ? visitTabs : scholarTabs;
  const filters = new URLSearchParams();
  const immersive =
    lens === "visit"
      ? view === "map" || view === "heritage" || view === "now"
      : view === "timeline" || view === "map";
  for (const key of ["from", "to", "types"]) {
    const value = params.get(key);
    if (value && !(view === "timeline" && (key === "from" || key === "to"))) filters.set(key, value);
  }
  const filterString = filters.toString();
  const activeRequest = useRef("");
  activeRequest.current = `${entityId}/${view}?${filterString}&provider=${provider}`;
  const eventId = params.get("event");
  const from = Number(params.get("from") ?? overview?.time_bounds.from ?? 0);
  const to = Number(params.get("to") ?? overview?.time_bounds.to ?? from + 1);
  const resolution = "detail";
  useEffect(() => { setOverview(undefined); setProviders([]); }, [entityId]);
  useEffect(() => {
    if (!entityId || lens !== "visit") return;
    if (!visitTabs.includes(view as (typeof visitTabs)[number])) {
      navigate(`/entities/${entityId}/map?${params.toString()}`, { replace: true });
    }
  }, [entityId, lens, view, navigate, params]);
  useEffect(() => {
    const controller = new AbortController();
    setError("");
    if (!entityId) return;
    getEntityView<EntityOverview>(
      entityId,
      "overview",
      new URLSearchParams(),
      controller.signal,
    )
      .then(setOverview)
      .catch((e: unknown) => {
        if (!controller.signal.aborted) setError(String(e));
      });
    return () => controller.abort();
  }, [entityId, revision]);
  useEffect(() => {
    const controller = new AbortController();
    setEvents([]);
    setSources([]);
    setNext(null);
    setLoading(Boolean(entityId));
    setError("");
    if (!entityId) return;
    const request = async () => {
      if (view === "sources") {
        const result = await fetchEntityBibliography(entityId, {
          limit: 100,
          providers: provider,
          signal: controller.signal,
        });
        if (!controller.signal.aborted) {
          setProviders(result.providers ?? []);
          setSources(result.items);
          setNext(result.next_cursor ?? null);
        }
      } else if (lens === "visit" && view === "now") {
        const result = await fetchVisitNow(entityId, { signal: controller.signal });
        if (!controller.signal.aborted) {
          setVisitNow(result.opportunities);
          setEvents([]);
        }
      } else if (lens === "visit") {
        const result = await fetchVisitHeritage(entityId, { signal: controller.signal });
        if (!controller.signal.aborted) {
          setEvents(result.events);
          setVisitNow([]);
        }
      } else {
        const query = new URLSearchParams(filterString);
        query.set("limit", view === "overview" ? "400" : "200");
        query.set(
          "resolution",
          view === "overview" ? "period" : resolution,
        );
        const collected: TimelineEvent[] = [];
        const paginateAll = view === "timeline" || view === "map";
        do {
          const result = await getEntityView<Page>(entityId, "timeline", query, controller.signal);
          if (controller.signal.aborted) break;
          collected.push(...result.events);
          setEvents([...collected]);
          setNext(result.pagination.next_cursor);
          if (view === "overview" || !paginateAll || !result.pagination.next_cursor) break;
          query.set("cursor", result.pagination.next_cursor);
        } while (!controller.signal.aborted);
      }
    };
    void request()
      .catch((e: unknown) => {
        if (!controller.signal.aborted) setError(String(e));
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [entityId, view, filterString, resolution, provider, revision, lens]);
  useEffect(() => {
    let cancelled = false;
    setSelected(undefined);
    if (eventId)
      void fetchEventDetail(eventId)
        .then((result) => {
          if (!cancelled && result.event?.entity_id === entityId)
            setSelected(result.event);
        })
        .catch((e: unknown) => {
          if (!cancelled) setError(String(e));
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
  const focusMoment = (event: TimelineEvent) => {
    select(event.id);
    if (lens === "visit") {
      const coords = event.coordinates;
      if (coords && Number.isFinite(coords.lat) && Number.isFinite(coords.lon)) {
        setMapFocus({ eventId: event.id, lat: coords.lat, lon: coords.lon });
      }
      if (view === "heritage") {
        const copy = new URLSearchParams(params);
        copy.set("event", event.id);
        navigate(`/entities/${entityId}/map?${copy.toString()}`);
      }
      return;
    }
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
  const update = (key: string, value: string) => {
    const copy = new URLSearchParams(params);
    if (value) copy.set(key, value);
    else copy.delete(key);
    copy.delete("event");
    setParams(copy);
  };
  const loadMore = async () => {
    if (!next || loading) return;
    setLoading(true);
    const requestKey = activeRequest.current;
    const query = new URLSearchParams(filterString);
    query.set("cursor", next);
    query.set("resolution", resolution);
    try {
      if (view === "sources") {
        const result = await fetchEntityBibliography(entityId, {
          limit: 100,
          providers: provider,
          cursor: next,
        });
        if (requestKey === activeRequest.current) {
          setSources((old) => [...old, ...result.items]);
          setNext(result.next_cursor ?? null);
        }
      } else {
        const result = await getEntityView<Page>(entityId, "timeline", query);
        if (requestKey === activeRequest.current) {
          setEvents((old) => [...old, ...result.events]);
          setNext(result.pagination.next_cursor);
        }
      }
    } catch (e) {
      if (requestKey === activeRequest.current) setError(String(e));
    } finally {
      if (requestKey === activeRequest.current) setLoading(false);
    }
  };
  const chronPreviewEvents = useMemo(
    () =>
      lens === "scholar" && view === "overview"
        ? selectChronologicalPreview(events, 12)
        : [],
    [events, lens, view],
  );
  return (
    <div
      className={`v3-shell${immersive ? " v3-shell--immersive" : ""}${scholarImmersive ? " v3-shell--scholar-stage" : ""}${mobileStage ? " v3-shell--mobile-stage" : ""}`}
    >
      {mobileStage ? (
        <header className="v3-mobile-chrome">
          <Link className="v3-mobile-chrome__back" to={`/entities/${entityId}/overview`}>
            {t.immersiveBack}
          </Link>
          <h1 className="v3-mobile-chrome__title">{overview?.entity.label ?? "Loading…"}</h1>
          <button
            type="button"
            className="v3-mobile-chrome__tools"
            aria-expanded={toolsOpen}
            onClick={() => setToolsOpen((open) => !open)}
          >
            {t.immersiveTools}
          </button>
        </header>
      ) : (
        <Navbar />
      )}
      <main className={`v3-main${immersive ? " v3-main--immersive" : ""}`}>
        {!immersive && (
          <section className={view === "overview" ? "v3-overview-hero" : undefined}>
            <div className="v3-heading">
              <div>
                <p className="v3-eyebrow">TALARIA · EXPLORER</p>
                <h1>
                  <span className="talaria-heading-with-mark">
                    <TalariaSandalMark />
                    {overview?.entity.label ?? (entityId ? "Loading…" : "Overview")}
                  </span>
                </h1>
                <p>Explore a life through time, places and evidence.</p>
              </div>
            </div>
            <section className="v3-search" aria-label="Search a person">
              <EntitySearchBox
                suggestions={picker.suggestions}
                onSubmitQuery={picker.setSearchQuery}
                isLoading={picker.searchLoading}
                onSelect={(item) => {
                  pendingSelection.current = true;
                  picker.selectPerson(item);
                  const selectedId = useExplorerStore.getState().entityId;
                  if (selectedId) {
                    pendingSelection.current = false;
                    navigate(`/entities/${selectedId}/map`);
                  }
                }}
              />
              {picker.ingestBusy && (
                <p role="status">
                  Collecting sources · {picker.timelineEvents} events · {picker.mapPins} map points
                </p>
              )}
              {picker.error && <p role="alert">{picker.error}</p>}
              {!entityId && !picker.ingestBusy && (
                <p className="v3-note">Search for a person to explore their life, places and sources.</p>
              )}
            </section>
          </section>
        )}
        {immersive && entityId && !mobileStage && (
          <div className="v3-immersive-bar">
            <h1 className="v3-immersive-title">
              {overview?.entity.label ?? "Loading…"}
            </h1>
            <VisitLensToggle entityId={entityId} view={view} />
          </div>
        )}
        {entityId && !immersive && (
          <div className="v3-lens-row">
            <VisitLensToggle entityId={entityId} view={view} />
          </div>
        )}
        {entityId && (
          <nav className="v3-tabs" aria-label="Entity views">
            {tabs.map((tab) => (
              <NavLink
                key={tab}
                to={`/entities/${entityId}/${tab}?${params}`}
                className={({ isActive }) => (isActive ? "active" : "")}
              >
                {tab === "heritage"
                  ? t.visitHeritageTab
                  : tab === "now"
                    ? t.visitNowTab
                    : tab}
              </NavLink>
            ))}
          </nav>
        )}
        {error && <p className="v3-inline-alert" role="alert">{error}</p>}
        {lens === "scholar" && view === "overview" && overview && (
          <section className="v3-stats">
            {Object.entries(overview.stats).map(([key, value]) => (
              <article key={key}>
                <strong>{value.toLocaleString()}</strong>
                <span>{key.replaceAll("_", " ")}</span>
              </article>
            ))}
          </section>
        )}
        <div
          className={
            immersive &&
            ((lens === "scholar" && (view === "map" || view === "timeline")) ||
              (lens === "visit" && (view === "map" || view === "heritage" || view === "now")))
              ? "v3-explorer-stage"
              : undefined
          }
        >
          {entityId &&
            ((lens === "scholar" && (view === "map" || view === "timeline")) ||
              (lens === "visit" && (view === "map" || view === "heritage" || view === "now"))) && (
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
              <EventMomentSearch
                events={events}
                loading={loading}
                onPick={focusMoment}
              />
              {view === "map" && lens === "scholar" && (
                <form className="v3-filters v3-filters--compact" onSubmit={(e) => e.preventDefault()}>
                  <label>
                    From{" "}
                    <input
                      type="number"
                      value={params.get("from") ?? ""}
                      onChange={(e) => update("from", e.target.value)}
                    />
                  </label>
                  <label>
                    To{" "}
                    <input
                      type="number"
                      value={params.get("to") ?? ""}
                      onChange={(e) => update("to", e.target.value)}
                    />
                  </label>
                </form>
              )}
            </div>
            )}
            </>
          )}
          {view === "map" && lens === "scholar" && (
            <EntityMap
              id={entityId}
              filters={filterString}
              selected={eventId ?? undefined}
              focus={mapFocus}
              onSelect={select}
              mode={stageMode}
            />
          )}
          {view === "map" && lens === "visit" && (
            <EntityVisitMap
              id={entityId}
              selected={eventId ?? undefined}
              focus={mapFocus}
              onSelect={select}
            />
          )}
          {view === "heritage" && lens === "visit" && (
            <VisitHeritageList
              events={events}
              loading={loading}
              emptyLabel={t.visitHeritageEmpty}
              onSelect={(id) => {
                const event = events.find((row) => row.id === id);
                if (event) focusMoment(event);
                else select(id);
              }}
            />
          )}
          {view === "now" && lens === "visit" && (
            <VisitNowList
              items={visitNow}
              loading={loading}
              emptyLabel={t.visitNowEmpty}
              onSelect={(id) => {
                const item = visitNow.find((row) => row.id === id);
                if (item?.coordinates) {
                  setMapFocus({
                    eventId: item.id,
                    lat: item.coordinates.lat,
                    lon: item.coordinates.lon,
                  });
                }
                select(id);
                const copy = new URLSearchParams(params);
                copy.set("event", id);
                navigate(`/entities/${entityId}/map?${copy.toString()}`);
              }}
            />
          )}
          {view === "timeline" && lens === "scholar" && overview && (
            <HistoricalTimeline
              key={entityId}
              events={events}
              bounds={[overview.time_bounds.from ?? from, overview.time_bounds.to ?? to]}
              from={from}
              to={to}
              spotlightEventId={eventId ?? undefined}
              zoomToEventId={timelineZoomEventId}
              zoomToEventToken={timelineZoomToken}
              mode={stageMode}
              onSelect={select}
              onZoom={(a, b) => {
                const copy = new URLSearchParams(params);
                copy.set("from", String(a));
                copy.set("to", String(b));
                setParams(copy, { replace: true });
              }}
            />
          )}
          {view === "timeline" && next && (
            <button
              className="v3-load-more"
              type="button"
              disabled={loading}
              onClick={() => void loadMore()}
            >
              Load more moments
            </button>
          )}
        </div>
        {lens === "scholar" && view === "overview" && (
          <section className="v3-chron-preview">
            <h2>{t.chronPreviewTitle}</h2>
            <p className="v3-note">{t.chronPreviewHint}</p>
            <ol className="v3-events v3-events--landmarks">
              {chronPreviewEvents.map((event) => (
                <li key={event.id}>
                  <span>{eventDate(event)}</span>
                  <button type="button" onClick={() => select(event.id)}>
                    {event.title}
                  </button>
                  <small>
                    {eventTypeLabel(event.event_type, locale)} ·{" "}
                    {event.place_label ?? "Place not established"}
                  </small>
                </li>
              ))}
            </ol>
            {!chronPreviewEvents.length && !loading && (
              <p>{t.emptyTimeline}</p>
            )}
          </section>
        )}
        {lens === "scholar" && view === "sources" && (
          <section>
            <h2>Source documents</h2>
            <p className="v3-note">
              Catalog matches are not proof of an event. Open an event to
              inspect its supporting evidence.
            </p>
            <div className="v3-provider-tabs" role="group" aria-label="Filter by database">
              {[{name: "", count: providers.reduce((sum, item) => sum + item.count, 0)}, ...databaseTabs(providers)].map((item) =>
                <button key={item.name} aria-pressed={provider === item.name} onClick={() => update("provider", item.name)}>
                  {item.name ? providerLabel(item.name) : "All databases"} <span>{item.count}</span>
                </button>)}
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
                    <a
                      href={source.canonical_url}
                      target="_blank"
                      rel="noreferrer"
                    >
                      Open source ↗
                    </a>
                  )}
                </article>
              ))}
            </div>
            {!sources.length && !loading && (
              <p>No bibliographic documents indexed for this database and person yet. Event evidence remains available in each event’s details.</p>
            )}
            {next && (
              <button disabled={loading} onClick={() => void loadMore()}>
                Load more sources
              </button>
            )}
          </section>
        )}
        {loading && <p role="status">Loading…</p>}
        {selected && (
          <aside className="v3-detail">
            <div className="v3-detail-links">
              {selected.map_eligible && (
                <Link to={`/entities/${entityId}/map?${params}`}>
                  View on map
                </Link>
              )}
              <Link to={`/entities/${entityId}/timeline?${params}`}>
                View in timeline
              </Link>
            </div>
            <EventDetailCard
              event={selected}
              onClose={() => select()}
            />
          </aside>
        )}

      </main>
    </div>
  );
}

function providerLabel(name: string): string {
  const labels: Record<string, string> = { wikipedia: "Wikipedia", wikidata: "Wikidata", hal: "HAL", bnf: "BnF", gallica: "Gallica", persee: "Persée", openalex: "OpenAlex", open_alex: "OpenAlex", openlibrary: "Open Library", open_library: "Open Library", internet_archive: "Internet Archive", theses_fr: "theses.fr", europeana: "Europeana", commons: "Wikimedia Commons", wikimedia_commons: "Wikimedia Commons", wikisource: "Wikisource" };
  return labels[name] ?? name.replaceAll("_", " ").replace(/\b\w/g, letter => letter.toUpperCase());
}

export function databaseTabs(providers: {name: string; count: number}[]) {
  const names = ["wikipedia", "wikidata", "open_alex", "persee", "hal", "gallica", "bnf", "theses_fr", "open_library", "internet_archive", "europeana", "wikisource", "wikimedia_commons", "crossref", "open_edition", "sudoc", "france_archives", "deutsche_biographie", "pop_merimee"];
  return [...new Set([...names, ...providers.map(p => p.name)])].map(name => ({ name, count: providers.find(p => p.name === name)?.count ?? 0 }));
}
