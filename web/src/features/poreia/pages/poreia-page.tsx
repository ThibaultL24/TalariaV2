// web/src/features/poreia/pages/poreia-page.tsx
import { lazy, Suspense, useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { EventDetailCard } from "@/components/detail/event-detail-card";
import { rememberEntity, useExperienceSubject } from "@/components/entity/entity-context";
import { ExperienceLayout } from "@/components/layout/experience-layout";
import { EntitySearchBox } from "@/components/search/entity-search-box";
import { EventMomentSearch } from "@/components/search/event-moment-search";
import { PoreiaPlaceCard } from "@/features/poreia/components/poreia-place-card";
import { placesFromMapFeatures } from "@/features/poreia/heritage-catalog";
import { PoreiaHeritageView } from "@/features/visit/poreia-heritage-view";
import { PoreiaItineraryView } from "@/features/visit/poreia-itinerary-view";
import { VisitNowList } from "@/features/visit/visit-now-list";
import { fetchEventDetail, type TimelineEvent } from "@/lib/api";
import {
  fetchVisitHeritage,
  fetchVisitNow,
  fetchVisitSites,
  getEntityView,
  type EntityOverview,
  type VisitOpportunity,
  type VisitSite,
} from "@/lib/entity-views";
import type { TalariaFeature } from "@/lib/schemas/geojson";
import { useI18n } from "@/lib/i18n";

const EntityVisitMap = lazy(() => import("@/features/visit/entity-visit-map").then((mod) => ({ default: mod.EntityVisitMap })));

export function PoreiaPage() {
  const { t, locale } = useI18n();
  const {
    entityId,
    view,
    params,
    setParams,
    navigate,
    picker,
    revision,
    pendingSelection,
    openPerson,
  } = useExperienceSubject("poreia");
  const overviewQuery = useQuery({
    queryKey: ["entity", entityId, "overview", revision],
    enabled: Boolean(entityId),
    retry: false,
    staleTime: 30_000,
    refetchOnWindowFocus: false,
    queryFn: ({ signal }) => getEntityView<EntityOverview>(entityId, "overview", new URLSearchParams(), signal),
  });
  const sitesQuery = useQuery({
    queryKey: ["entity", entityId, "visit-sites", revision],
    enabled: Boolean(entityId) && view === "heritage",
    retry: false,
    refetchOnWindowFocus: false,
    queryFn: ({ signal }) => fetchVisitSites(entityId, { signal }),
  });
  const heritageQuery = useQuery({
    queryKey: ["entity", entityId, "visit-heritage", revision, locale],
    enabled: Boolean(entityId) && view === "heritage",
    retry: false,
    refetchOnWindowFocus: false,
    queryFn: ({ signal }) => fetchVisitHeritage(entityId, { signal, lang: locale }),
  });
  const nowQuery = useQuery({
    queryKey: ["entity", entityId, "visit-now", revision],
    enabled: Boolean(entityId) && view === "now",
    retry: false,
    refetchOnWindowFocus: false,
    queryFn: ({ signal }) => fetchVisitNow(entityId, { signal }),
  });
  const overview = overviewQuery.data;
  const visitSites: VisitSite[] = sitesQuery.data?.sites ?? [];
  const events: TimelineEvent[] = heritageQuery.data?.events ?? [];
  const visitNow: VisitOpportunity[] = nowQuery.data?.opportunities ?? [];

  useEffect(() => {
    if (!entityId) return;
    rememberEntity(entityId, overview?.entity.label, overview?.entity.qid);
  }, [entityId, overview]);

  const immersive = Boolean(entityId);
  const [toolsOpen, setToolsOpen] = useState(false);
  const [mapFocus, setMapFocus] = useState<{ eventId: string; lat: number; lon: number } | undefined>();
  const [placeKind, setPlaceKind] = useState("");
  const [mapView, setMapView] = useState<{ features: TalariaFeature[]; loading: boolean }>({ features: [], loading: true });
  const reportMapView = useCallback((view: { features: TalariaFeature[]; loading: boolean }) => {
    setMapView(view);
  }, []);
  const mapPlaces = useMemo(() => placesFromMapFeatures(mapView.features), [mapView.features]);
  const [selected, setSelected] = useState<TimelineEvent>();
  const eventId = params.get("event");
  const showTools = view === "map" || view === "heritage" || view === "now";

  useEffect(() => {
    let cancelled = false;
    setSelected(undefined);
    if (!eventId) return;
    void fetchEventDetail(eventId, locale)
      .then((result) => {
        if (!cancelled && result.event?.entity_id === entityId) setSelected(result.event);
      })
      .catch(() => {
        if (!cancelled) setSelected(undefined);
      });
    return () => {
      cancelled = true;
    };
  }, [eventId, entityId, locale]);

  const select = (id?: string) => {
    const copy = new URLSearchParams(params);
    if (id) copy.set("event", id);
    else copy.delete("event");
    setParams(copy);
  };
  const loading = heritageQuery.isFetching || nowQuery.isFetching;
  const error = heritageQuery.error
    ? String(heritageQuery.error)
    : nowQuery.error
      ? String(nowQuery.error)
      : overviewQuery.error
        ? String(overviewQuery.error)
        : "";

  return (
    <ExperienceLayout
      experience="poreia"
      entityId={entityId}
      label={overview?.entity.label}
      immersive={immersive}
      mobileStage={false}
      toolsOpen={toolsOpen}
      onToggleTools={() => setToolsOpen((open) => !open)}
      showHeroBand={!entityId}
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
            <>
              <img className="poreia-entry__scene" src="/brand/poreia-scene.webp" alt={t.itineraryBanner} />
              <p className="v3-note">{t.poreiaChoosePerson}</p>
            </>
          )}
        </>
      }
    >
      {error && <p className="v3-inline-alert" role="alert">{error}</p>}
      <div className={entityId && showTools ? "v3-explorer-stage" : undefined}>
        {entityId && showTools && (
          <div className="v3-moment-search-row">
            <EventMomentSearch
              events={view === "now" ? [] : events}
              loading={loading}
              onPick={(event) => {
                select(event.id);
                const coords = event.coordinates;
                if (coords && Number.isFinite(coords.lat) && Number.isFinite(coords.lon)) {
                  setMapFocus({ eventId: event.id, lat: coords.lat, lon: coords.lon });
                }
                if (view !== "map") {
                  const copy = new URLSearchParams(params);
                  copy.set("event", event.id);
                  navigate(`/poreia/${entityId}/map?${copy.toString()}`);
                }
              }}
            />
          </div>
        )}
        {view === "map" && entityId && (
          <div className="poreia-workspace">
            <Suspense fallback={<p role="status">{t.loading}</p>}>
            <EntityVisitMap
              id={entityId}
              selected={mapFocus?.eventId ?? eventId ?? undefined}
              focus={mapFocus}
              onView={reportMapView}
              onSelect={(id) => {
                const place = mapPlaces.find((row) => row.id === id);
                if (place?.coordinates) {
                  setMapFocus({ eventId: id, lat: place.coordinates.lat, lon: place.coordinates.lon });
                }
                if (!place || place.origins.includes("heritage_event")) select(id);
              }}
            />
            </Suspense>
            <aside className="poreia-sheet" aria-label={t.visitHeritageTab}>
              <PlaceFilters places={mapPlaces} kind={placeKind} onKind={setPlaceKind} />
              <ul className="visit-heritage-list">
                {mapPlaces.filter((place) => !placeKind || place.kind === placeKind).map((place) => (
                  <li key={place.id}>
                    <button
                      type="button"
                      aria-current={place.id === (mapFocus?.eventId ?? eventId) ? "true" : undefined}
                      onClick={() => {
                        if (!place.coordinates) return;
                        setMapFocus({ eventId: place.id, lat: place.coordinates.lat, lon: place.coordinates.lon });
                        if (place.origins.includes("heritage_event")) select(place.id);
                      }}
                    >
                      {place.title}
                    </button>
                  </li>
                ))}
              </ul>
              {mapPlaces.length === 0 && !mapView.loading && (
                <p role="status">{t.noPlacesInView}</p>
              )}
              {mapPlaces.filter((place) => place.id === (mapFocus?.eventId ?? eventId)).map((place) => (
                <PoreiaPlaceCard key={place.id} place={place} />
              ))}
              <Link className="button button--ghost" to={`/poreia/${entityId}/itinerary`}>{t.seeItinerary}</Link>
            </aside>
          </div>
        )}
        {view === "heritage" && entityId && (
          <PoreiaHeritageView
            entityId={entityId}
            sites={visitSites}
            events={events}
            loading={heritageQuery.isFetching}
            onFocusSite={(site) => {
              setMapFocus({ eventId: site.id, lat: site.coordinates.lat, lon: site.coordinates.lon });
              const copy = new URLSearchParams(params);
              copy.delete("event");
              navigate(`/poreia/${entityId}/map?${copy.toString()}`);
            }}
            onFocusEvent={(id) => {
              const event = events.find((row) => row.id === id);
              if (event?.coordinates) {
                setMapFocus({ eventId: id, lat: event.coordinates.lat, lon: event.coordinates.lon });
              }
              const copy = new URLSearchParams(params);
              copy.set("event", id);
              navigate(`/poreia/${entityId}/map?${copy.toString()}`);
            }}
          />
        )}
        {view === "itinerary" && entityId && (
          <PoreiaItineraryView
            entityId={entityId}
            onFocus={(point) => {
              setMapFocus({ eventId: point.id, lat: point.lat, lon: point.lon });
              navigate(`/poreia/${entityId}/map`);
            }}
          />
        )}
        {view === "now" && entityId && (
          <VisitNowList
            items={visitNow}
            loading={nowQuery.isFetching}
            emptyLabel={t.visitNowEmpty}
            onSelect={(id) => {
              const item = visitNow.find((row) => row.id === id);
              if (item?.coordinates) {
                setMapFocus({ eventId: item.id, lat: item.coordinates.lat, lon: item.coordinates.lon });
              }
              const copy = new URLSearchParams(params);
              copy.set("event", id);
              navigate(`/poreia/${entityId}/map?${copy.toString()}`);
            }}
          />
        )}
      </div>
      {loading && <p role="status">{t.loading}</p>}
      {selected && (
        <aside className="v3-detail">
          <EventDetailCard event={selected} onClose={() => select()} />
        </aside>
      )}
    </ExperienceLayout>
  );
}

function PlaceFilters({
  places,
  kind,
  onKind,
}: {
  places: { kind: string }[];
  kind: string;
  onKind: (kind: string) => void;
}) {
  const { t } = useI18n();
  const kinds = [...new Set(places.map((place) => place.kind))];
  if (kinds.length < 2) return null;
  return (
    <div className="poreia-filters" role="group" aria-label={t.filterByPlaceKind}>
      <button type="button" aria-pressed={kind === ""} onClick={() => onKind("")}>{t.allKinds}</button>
      {kinds.map((value) => (
        <button key={value} type="button" aria-pressed={kind === value} onClick={() => onKind(value)}>
          {value.replaceAll("_", " ")}
        </button>
      ))}
    </div>
  );
}

