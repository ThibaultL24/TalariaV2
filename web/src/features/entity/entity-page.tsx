import { useEffect, useRef, useState } from "react";
import { Link, NavLink, useParams, useSearchParams } from "react-router-dom";
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
  const { entityId = "", view = "overview" } = useParams();
  const [params, setParams] = useSearchParams();
  const [overview, setOverview] = useState<EntityOverview>();
  const [events, setEvents] = useState<TimelineEvent[]>([]);
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
    if (value) filters.set(key, value);
  }
  const filterString = filters.toString();
  const activeRequest = useRef("");
  activeRequest.current = `${entityId}/${view}?${filterString}`;
  const eventId = params.get("event");
  const from = Number(params.get("from") ?? overview?.time_bounds.from ?? 0);
  const to = Number(params.get("to") ?? overview?.time_bounds.to ?? from + 1);
  const resolution =
    to - from > 40 ? "overview" : to - from > 10 ? "period" : "detail";
  useEffect(() => {
    const controller = new AbortController();
    setOverview(undefined);
    setError("");
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
  }, [entityId]);
  useEffect(() => {
    const controller = new AbortController();
    setEvents([]);
    setSources([]);
    setNext(null);
    setLoading(true);
    setError("");
    const request = async () => {
      if (view === "sources") {
        const result = await fetchEntityBibliography(entityId, {
          limit: 100,
          signal: controller.signal,
        });
        if (!controller.signal.aborted) {
          setSources(result.items);
          setNext(result.next_cursor ?? null);
        }
      } else if (view !== "map") {
        const query = new URLSearchParams(filterString);
        query.set("limit", "200");
        query.set("resolution", view === "overview" ? "overview" : resolution);
        const result = await getEntityView<Page>(
          entityId,
          "timeline",
          query,
          controller.signal,
        );
        if (!controller.signal.aborted) {
          setEvents(result.events);
          setNext(result.pagination.next_cursor);
        }
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
  }, [entityId, view, filterString, resolution]);
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
            <h1>{overview?.entity.label ?? "Loading…"}</h1>
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
        <nav className="v3-tabs" aria-label="Entity views">
          {["overview", "timeline", "map", "sources"].map((tab) => (
            <NavLink
              key={tab}
              to={`/entities/${entityId}/${tab}?${params}`}
              className={({ isActive }) => (isActive ? "active" : "")}
            >
              {tab}
            </NavLink>
          ))}
        </nav>
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
        {view === "timeline" && overview?.time_bounds.from != null && (
          <TimelineCanvas
            events={events}
            from={from}
            to={to}
            onSelect={select}
            onZoom={(a, b) => {
              const copy = new URLSearchParams(params);
              copy.set("from", String(a));
              copy.set("to", String(b));
              setParams(copy);
            }}
          />
        )}
        {(view === "overview" || view === "timeline") && (
          <section>
            <h2>
              {view === "overview" ? "Chronological preview" : "Timeline"}
            </h2>
            <p className="v3-note">
              Dates retain the precision and uncertainty recorded in the
              sources.
            </p>
            <ol className="v3-events">
              {(view === "overview" ? events.slice(0, 8) : events).map(
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
            {view === "timeline" && next && (
              <button disabled={loading} onClick={() => void loadMore()}>
                Load more
              </button>
            )}
          </section>
        )}
        {view === "sources" && (
          <section>
            <h2>Source documents</h2>
            <p className="v3-note">
              Catalog matches are not proof of an event. Open an event to
              inspect its supporting evidence.
            </p>
            <div className="v3-sources">
              {sources.map((source) => (
                <article key={source.id}>
                  <span>
                    {source.source_kind} · {source.document_type}
                  </span>
                  <h3>{source.title}</h3>
                  <p>
                    {source.academic_status} · {source.epistemic}
                  </p>
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
              <p>No source documents available.</p>
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
              showIntuition={false}
            />
          </aside>
        )}
        <Link to={`/explorer?entity=${entityId}`}>Open original explorer</Link>
      </main>
    </div>
  );
}
