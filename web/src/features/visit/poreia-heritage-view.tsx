// web/src/features/visit/poreia-heritage-view.tsx
import { useState } from "react";
import { Link } from "react-router-dom";
import type { TimelineEvent } from "@/lib/api";
import { PoreiaPlaceCard } from "@/features/poreia/components/poreia-place-card";
import { buildHeritageCatalog, type HeritagePlace } from "@/features/poreia/heritage-catalog";
import type { VisitSite } from "@/lib/entity-views";
import { useI18n } from "@/lib/i18n";

interface Props {
  entityId: string;
  sites: VisitSite[];
  events: TimelineEvent[];
  loading?: boolean;
  onFocusSite: (site: VisitSite) => void;
  onFocusEvent: (id: string) => void;
}

export function PoreiaHeritageView({
  entityId, sites, events, loading = false, onFocusSite, onFocusEvent,
}: Props) {
  const { locale, t } = useI18n();
  const places = buildHeritageCatalog(sites, events);
  const [kind, setKind] = useState<string>("");
  const visible = kind ? places.filter((place) => place.kind === kind) : places;
  const kinds = [...new Set(places.map((place) => place.kind))];

  const focus = (place: HeritagePlace) => {
    if (place.origins.includes("wikidata_site") && place.relationPid && place.wikidataQid && place.coordinates) {
      onFocusSite({
        id: place.id,
        wikidata_qid: place.wikidataQid,
        label: place.title,
        kind: place.kind as VisitSite["kind"],
        relation_pid: place.relationPid,
        coordinates: place.coordinates,
        visitability: place.visitability,
      });
      return;
    }
    onFocusEvent(place.id);
  };

  return (
    <section className="poreia-heritage" aria-labelledby="poreia-heritage-title">
      <header className="poreia-heritage__header">
        <p className="experience-section-kicker">POREIA · {locale === "fr" ? "PATRIMOINE" : "HERITAGE"}</p>
        <h2 id="poreia-heritage-title">{locale === "fr" ? "Les lieux de son histoire" : "Places connected to this life"}</h2>
        <p>{locale === "fr"
          ? "Résidences, monuments et lieux de mémoire liés à cette personnalité. L’existence d’un site ne garantit pas son ouverture au public."
          : "Residences, monuments and places of memory linked to this person. A documented site is not necessarily open to visitors."}</p>
      </header>
      {places.length > 0 ? (
        <>
          <p className="poreia-heritage__count">
            {locale === "fr"
              ? `${places.length} lieu${places.length > 1 ? "x" : ""} documenté${places.length > 1 ? "s" : ""}`
              : `${places.length} documented ${places.length === 1 ? "place" : "places"}`}
          </p>
          {kinds.length > 1 && (
            <div className="poreia-filters" role="group" aria-label={locale === "fr" ? "Filtrer par type de lieu" : "Filter by place type"}>
              <button type="button" aria-pressed={kind === ""} onClick={() => setKind("")}>
                {locale === "fr" ? "Tous" : "All"}
              </button>
              {kinds.map((value) => (
                <button key={value} type="button" aria-pressed={kind === value} onClick={() => setKind(value)}>
                  {value.replaceAll("_", " ")}
                </button>
              ))}
            </div>
          )}
          <ul className="visit-heritage-list">
            {visible.map((place) => (
              <li key={`${place.id}:${place.origins.join("-")}`}>
                <button type="button" onClick={() => focus(place)}>
                  <span className="visit-heritage-list__title">{place.title}</span>
                  {place.city ? <span className="visit-heritage-list__place">{place.city}</span> : null}
                </button>
                <PoreiaPlaceCard place={place} heading={false} />
              </li>
            ))}
          </ul>
        </>
      ) : loading ? (
        <p role="status" className="poreia-heritage__status">{t.loading}</p>
      ) : (
        <div className="poreia-heritage__empty" role="status">
          <span className="poreia-heritage__empty-icon" aria-hidden="true">⌖</span>
          <h3>{locale === "fr" ? "Les lieux restent à documenter" : "Places are not indexed yet"}</h3>
          <p>{locale === "fr"
            ? "Aucun site patrimonial lié à cette personnalité n’est encore disponible. Consultez sa carte de vie dans Istoria en attendant l’enrichissement."
            : "No heritage site linked to this person is available yet. Explore the sourced life map in Istoria while the catalog grows."}</p>
          <Link className="button button--ghost" to={`/istoria/${entityId}/map`}>
            {locale === "fr" ? "Voir la carte de vie dans Istoria ↗" : "Open the life map in Istoria ↗"}
          </Link>
        </div>
      )}
    </section>
  );
}
