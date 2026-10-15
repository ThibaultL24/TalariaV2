// web/src/features/visit/visit-page.tsx
import { useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { Navbar } from "@/components/layout/navbar";
import { EntitySearchBox } from "@/components/search/entity-search-box";
import { usePersonPicker } from "@/hooks/use-person-picker";
import { useI18n } from "@/lib/i18n";
import { VisitMap } from "./visit-map";
import "@/styles/visit.css";

interface VisitSite {
  id: string;
  place_label: string;
  category: string;
  relation: string;
  temporal_class: string;
  visitability: string;
  summary?: string | null;
  provenance: { kind?: string; event_id?: string };
  coordinates?: { lat: number; lon: number } | null;
  image?: { thumb_url?: string; credit?: string; license?: string; commons_file?: string } | null;
}

interface VisitItemPage {
  items: VisitSite[];
}

interface Commemoration {
  id: string;
  title: string;
  occurrence_status: string;
  place_persistence: string;
  bookable: boolean;
}

interface ItineraryStop {
  ordinal: number;
  site_id: string;
  straight_line_km: number;
  travel_minutes: number | null;
}

interface VisitFeatureCollection {
  type: "FeatureCollection";
  features: Array<{ type: "Feature"; properties?: { id?: string } }>;
}

const EMPTY_MAP: VisitFeatureCollection = { type: "FeatureCollection", features: [] };

export function VisitPage() {
  const { entityKey } = useParams();
  const navigate = useNavigate();
  const { t } = useI18n();
  const picker = usePersonPicker({ startLifeIngest: false });
  const [entityId, setEntityId] = useState(entityKey ?? "");
  const [sites, setSites] = useState<VisitSite[]>([]);
  const [memory, setMemory] = useState<Commemoration[]>([]);
  const [selected, setSelected] = useState<string>();
  const [picked, setPicked] = useState<string[]>([]);
  const [mode, setMode] = useState("walk");
  const [stops, setStops] = useState<ItineraryStop[]>([]);
  const [error, setError] = useState("");
  const [geojson, setGeojson] = useState<VisitFeatureCollection>(EMPTY_MAP);

  useEffect(() => {
    if (!entityKey) return;
    let cancelled = false;
    async function resolve() {
      const key = entityKey ?? "";
      if (/^[0-9a-f-]{36}$/i.test(key)) {
        if (!cancelled) setEntityId(key);
        return;
      }
      const response = await fetch(`/api/v1/entities/by-qid/${encodeURIComponent(key)}`);
      if (!response.ok) {
        if (!cancelled) setError(t.loadFailed);
        return;
      }
      const body = (await response.json()) as { entity_id: string };
      if (!cancelled) setEntityId(body.entity_id);
    }
    void resolve();
    return () => {
      cancelled = true;
    };
  }, [entityKey, t.loadFailed]);

  useEffect(() => {
    if (!entityId) return;
    let cancelled = false;
    async function load() {
      const [sitePage, mapPage, memoryPage] = await Promise.all([
        fetch(`/api/v1/entities/${entityId}/visit/sites?limit=50`),
        fetch(`/api/v1/entities/${entityId}/visit/map`),
        fetch(`/api/v1/entities/${entityId}/visit/commemorations?limit=20`),
      ]);
      if (!sitePage.ok || !mapPage.ok || !memoryPage.ok) {
        if (!cancelled) setError(t.loadFailed);
        return;
      }
      const siteBody = (await sitePage.json()) as VisitItemPage;
      const mapBody = (await mapPage.json()) as VisitFeatureCollection;
      const memoryBody = (await memoryPage.json()) as { items: Commemoration[] };
      if (cancelled) return;
      setSites(siteBody.items);
      setGeojson(mapBody);
      setMemory(memoryBody.items);
      setError("");
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, [entityId, t.loadFailed]);

  const selectedSite = sites.find((site) => site.id === selected) ?? sites[0];
  const counts = useMemo(() => {
    const tally = new Map<string, number>();
    for (const site of sites) tally.set(site.category, (tally.get(site.category) ?? 0) + 1);
    return [...tally.entries()];
  }, [sites]);

  async function discover() {
    const response = await fetch(`/api/v1/entities/${entityId}/visit/discover`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ live: true }),
    });
    if (!response.ok) {
      setError(t.loadFailed);
      return;
    }
    const sitePage = await fetch(`/api/v1/entities/${entityId}/visit/sites?limit=50`);
    const mapPage = await fetch(`/api/v1/entities/${entityId}/visit/map`);
    if (sitePage.ok) setSites(((await sitePage.json()) as VisitItemPage).items);
    if (mapPage.ok) setGeojson((await mapPage.json()) as VisitFeatureCollection);
  }

  async function project() {
    const response = await fetch(`/api/v1/entities/${entityId}/visit/project`, { method: "POST" });
    if (!response.ok) {
      setError(t.loadFailed);
      return;
    }
    setEntityId((current) => current);
    const sitePage = await fetch(`/api/v1/entities/${entityId}/visit/sites?limit=50`);
    const mapPage = await fetch(`/api/v1/entities/${entityId}/visit/map`);
    if (sitePage.ok) setSites(((await sitePage.json()) as VisitItemPage).items);
    if (mapPage.ok) setGeojson((await mapPage.json()) as VisitFeatureCollection);
  }

  function toggleStop(id: string) {
    setPicked((current) => (current.includes(id) ? current.filter((item) => item !== id) : [...current, id]));
  }

  async function plan() {
    const response = await fetch("/api/v1/visit/itineraries", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ entity_id: entityId, mode, site_ids: picked }),
    });
    if (!response.ok) {
      setError(t.loadFailed);
      return;
    }
    const body = (await response.json()) as { stops: ItineraryStop[] };
    setStops(body.stops);
  }

  return (
    <div className="visit-shell">
      <Navbar
        center={
          <EntitySearchBox
            suggestions={picker.suggestions}
            onSubmitQuery={picker.setSearchQuery}
            onSelect={(item) => {
              const id = picker.selectPerson(item);
              if (id) navigate(`/visit/${id}`);
            }}
            isLoading={picker.searchLoading}
          />
        }
      />
      <header className="visit-hero">
        <p className="visit-meta">{t.visitEyebrow}</p>
        <h1>{t.visit}</h1>
        <p>{t.visitSubtitle}</p>
      </header>
      <div className="visit-layout">
        <VisitMap geojson={geojson} selectedId={selectedSite?.id} onSelect={setSelected} />
        <section className="visit-sheet" aria-label="Visit results">
          {error ? <p role="alert">{error}</p> : null}
          {entityId ? (
            <div className="visit-actions">
              <button type="button" onClick={() => void project()}>
                {t.visitProject}
              </button>
              <button type="button" onClick={() => void discover()}>
                {t.visitDiscover}
              </button>
            </div>
          ) : (
            <p>{t.emptySearch}</p>
          )}
          <p className="visit-meta">{counts.map(([name, count]) => `${name} ${count}`).join(" · ")}</p>
          {sites.length === 0 && entityId ? <p>{t.visitEmpty}</p> : null}
          {sites.map((site) => (
            <article key={site.id} className="visit-card">
              <h2>{site.place_label}</h2>
              {site.image?.thumb_url ? (
                <figure>
                  <img src={site.image.thumb_url} alt={site.place_label} />
                  <figcaption className="visit-meta">
                    {site.image.credit} · {site.image.license}
                  </figcaption>
                </figure>
              ) : null}
              <p className="visit-meta">
                {site.category} · {site.relation} · {site.temporal_class}
              </p>
              <p>{site.summary || site.relation}</p>
              <p className="visit-meta">
                {t.sources}: {site.provenance.kind ?? "source"} · visitability {site.visitability}
              </p>
              <div className="visit-actions">
                <button type="button" onClick={() => setSelected(site.id)} aria-pressed={selected === site.id}>
                  {site.place_label}
                </button>
                <button type="button" onClick={() => toggleStop(site.id)}>
                  {picked.includes(site.id) ? "−" : "+"} {site.place_label}
                </button>
                <Link to={`/entities/${entityId}/map`}>{t.openInScholar}</Link>
              </div>
            </article>
          ))}
          <section className="visit-panel">
            <h2>{t.visitLegacy}</h2>
            {memory.length === 0 ? <p className="visit-meta">{t.visitEmpty}</p> : null}
            {memory.map((item) => (
              <p key={item.id}>
                {item.title} · {item.occurrence_status} · {item.place_persistence}
                {item.bookable ? "" : " · not bookable"}
              </p>
            ))}
          </section>
          {stops.length > 0 ? (
            <ol>
              {stops.map((stop) => (
                <li key={stop.site_id}>
                  {stop.ordinal + 1}. {sites.find((site) => site.id === stop.site_id)?.place_label ?? stop.site_id} ·{" "}
                  {stop.straight_line_km.toFixed(1)} km · {stop.travel_minutes ?? "—"}
                </li>
              ))}
            </ol>
          ) : null}
          <p className="visit-meta">{t.visitRoutingNote}</p>
        </section>
      </div>
      <div className="visit-dock">
        <label>
          Mode{" "}
          <select value={mode} onChange={(event) => setMode(event.target.value)} aria-label="Travel mode">
            <option value="walk">walk</option>
            <option value="bike">bike</option>
            <option value="drive">drive</option>
          </select>
        </label>
        <button type="button" onClick={() => void plan()} disabled={!entityId || picked.length === 0}>
          Plan
        </button>
      </div>
    </div>
  );
}
