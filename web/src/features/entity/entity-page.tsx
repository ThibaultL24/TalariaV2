import { useEffect, useRef, useState } from "react";
import { Link, NavLink, useNavigate, useParams, useSearchParams } from "react-router-dom";
import { EntitySearchBox } from "@/components/search/entity-search-box";
import { usePersonPicker } from "@/hooks/use-person-picker";
import { useExplorerStore } from "@/stores/explorer-store";
import { Navbar } from "@/components/layout/navbar";
import { EventDetailCard } from "@/components/detail/event-detail-card";
import {
  fetchEntityBibliography,
  fetchEventDetail,
  type BibliographyItem,
  type TimelineEvent,
} from "@/lib/api";
import {
  getEntityView,
  eventDate,
  type EntityOverview,
  type Page,
} from "@/lib/entity-views";
import { useThemeStore } from "@/stores/theme-store";
import { TimelineCanvas } from "./timeline-canvas";
import { EntityMap } from "./entity-map";

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
      navigate(`/entities/${storedId}/overview`);
    } else if (!routeEntityId && entityId) {
      navigate(`/entities/${entityId}/overview`, { replace: true });
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
  const preference = useThemeStore((s) => s.preference);
  const setTheme = useThemeStore((s) => s.setTheme);
  const filters = new URLSearchParams();
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
      } else if (view !== "map") {
        const query = new URLSearchParams(filterString);
        query.set("limit", "200");
        query.set("resolution", view === "overview" ? "overview" : resolution);
        const collected: TimelineEvent[] = [];
        do {
          const result = await getEntityView<Page>(entityId, "timeline", query, controller.signal);
          if (controller.signal.aborted) break;
          collected.push(...result.events);
          setEvents([...collected]);
          setNext(result.pagination.next_cursor);
          if (view !== "timeline" || !result.pagination.next_cursor) break;
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
  }, [entityId, view, filterString, resolution, provider, revision]);
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
  return (
    <div className="v3-shell">
      <Navbar />
      <main className="v3-main">
        <div className="v3-heading">
          <div>
            <p className="v3-eyebrow">TALARIA · EXPLORER</p>
            <h1>{overview?.entity.label ?? (entityId ? "Loading…" : "Overview")}</h1>
            <p>Explore a life through time, places and evidence.</p>
          </div>
          <label>
            Appearance{" "}
            <select
              value={preference}
              onChange={(e) =>
                setTheme(e.target.value as "light" | "dark" | "system")
              }
            >
              <option value="light">Light</option>
              <option value="dark">Dark</option>
              <option value="system">System</option>
            </select>
          </label>
        </div>
        <section className="v3-search" aria-label="Search a person">
          <EntitySearchBox suggestions={picker.suggestions} onSubmitQuery={picker.setSearchQuery}
            isLoading={picker.searchLoading} onSelect={(item) => {
              pendingSelection.current = true;
              picker.selectPerson(item);
              const selectedId = useExplorerStore.getState().entityId;
              if (selectedId) { pendingSelection.current = false; navigate(`/entities/${selectedId}/overview`); }
            }} />
          {picker.ingestBusy && <p role="status">Collecting sources · {picker.timelineEvents} events · {picker.mapPins} map points</p>}
          {picker.error && <p role="alert">{picker.error}</p>}
          {!entityId && !picker.ingestBusy && <p className="v3-note">Search for a person to explore their life, places and sources.</p>}
        </section>
        {entityId && <nav className="v3-tabs" aria-label="Entity views">
          {["overview", "timeline", "map", "sources"].map((tab) => (
            <NavLink
              key={tab}
              to={`/entities/${entityId}/${tab}?${params}`}
              className={({ isActive }) => (isActive ? "active" : "")}
            >
              {tab}
            </NavLink>
          ))}
        </nav>}
        {error && <p role="alert">{error}</p>}
        {(view === "timeline" || view === "map") && (
          <form className="v3-filters" onSubmit={(e) => e.preventDefault()}>
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
            <label>
              Event types{" "}
              <input
                placeholder="residence,travel"
                value={params.get("types") ?? ""}
                onChange={(e) => update("types", e.target.value)}
              />
            </label>
          </form>
        )}
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
        {view === "map" && (
          <EntityMap
            id={entityId}
            filters={filterString}
            selected={eventId ?? undefined}
            onSelect={select}
          />
        )}
        {view === "timeline" && overview && (
          <TimelineCanvas
            key={entityId}
            events={events}
            bounds={[overview.time_bounds.from ?? from, overview.time_bounds.to ?? to]}
            from={from}
            to={to}
            onSelect={select}
            onZoom={(a, b) => {
              const copy = new URLSearchParams(params);
              copy.set("from", String(a));
              copy.set("to", String(b));
              setParams(copy, { replace: true });
            }}
          />
        )}
        {view === "overview" && (
          <section>
            <h2>
              Chronological preview
            </h2>
            <p className="v3-note">
              Dates retain the precision and uncertainty recorded in the
              sources.
            </p>
            <ol className="v3-events">
              {events.slice(0, 8).map(
                (event) => (
                  <li key={event.id}>
                    <span>{eventDate(event)}</span>
                    <button onClick={() => select(event.id)}>
                      {event.title}
                    </button>
                    <small>
                      {event.event_type.replaceAll("_", " ")} ·{" "}
                      {event.place_label ?? "Place not established"}
                    </small>
                  </li>
                ),
              )}
            </ol>
            {!events.length && !loading && (
              <p>No events match this selection.</p>
            )}
          </section>
        )}
        {view === "timeline" && next && <button disabled={loading} onClick={() => void loadMore()}>Load more moments</button>}
        {view === "sources" && (
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
