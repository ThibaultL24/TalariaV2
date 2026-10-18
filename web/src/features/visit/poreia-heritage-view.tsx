import { Link } from "react-router-dom";
import type { TimelineEvent } from "@/lib/api";
import type { VisitSite } from "@/lib/entity-views";
import { VisitSitesList } from "./visit-sites-list";
import { VisitHeritageList } from "./visit-heritage-list";
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
  const hasSites = sites.length > 0;
  const hasEvents = events.length > 0;

  return (
    <section className="poreia-heritage" aria-labelledby="poreia-heritage-title">
      <header className="poreia-heritage__header">
        <p className="experience-section-kicker">POREIA · {locale === "fr" ? "PATRIMOINE" : "HERITAGE"}</p>
        <h2 id="poreia-heritage-title">{locale === "fr" ? "Les lieux de son histoire" : "Places connected to this life"}</h2>
        <p>{locale === "fr"
          ? "Résidences, monuments et lieux de mémoire liés à cette personnalité. L’existence d’un site ne garantit pas son ouverture au public."
          : "Residences, monuments and places of memory linked to this person. A documented site is not necessarily open to visitors."}</p>
      </header>
      {hasSites ? (
        <>
          <p className="poreia-heritage__count">
            {locale === "fr"
              ? `${sites.length} lieu${sites.length > 1 ? "x" : ""} documenté${sites.length > 1 ? "s" : ""}`
              : `${sites.length} documented ${sites.length === 1 ? "place" : "places"}`}
          </p>
          <VisitSitesList sites={sites} onFocus={onFocusSite} />
        </>
      ) : hasEvents ? (
        <>
          <p className="poreia-heritage__count">{locale === "fr"
            ? "Lieux du catalogue historique"
            : "Places from the historical catalog"}</p>
          <VisitHeritageList events={events} loading={loading} onSelect={onFocusEvent} emptyLabel={t.visitHeritageEmpty} />
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
