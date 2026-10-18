import type { VisitSite } from "@/lib/entity-views";
import { useI18n } from "@/lib/i18n";

const RELATIONS: Record<VisitSite["relation_pid"], { fr: string; en: string }> = {
  P547: { fr: "Hommage", en: "Commemorates" },
  P138: { fr: "Nommé en l’honneur de", en: "Named after" },
  P119: { fr: "Sépulture", en: "Burial place" },
  P551: { fr: "Résidence", en: "Residence" },
};

const ACCESS: Record<VisitSite["visitability"], { fr: string; en: string }> = {
  unknown: { fr: "Accès non vérifié", en: "Access not verified" },
  verified_visitable: { fr: "Visite confirmée", en: "Visit verified" },
  verified_closed: { fr: "Fermé au public", en: "Closed to the public" },
  restricted: { fr: "Accès restreint", en: "Restricted access" },
};

export function VisitSitesList({
  sites, onFocus,
}: {
  sites: VisitSite[];
  onFocus: (site: VisitSite) => void;
}) {
  const { locale } = useI18n();
  if (!sites.length) return null;
  return (
    <section className="visit-wikidata-sites" aria-label={locale === "fr" ? "Sites patrimoniaux documentés" : "Documented heritage sites"}>
      <ul className="visit-heritage-list">
        {sites.map((site) => (
          <li key={`${site.id}:${site.relation_pid}`}>
            <button type="button" onClick={() => onFocus(site)}>
              <span className="visit-heritage-list__meta">
                {RELATIONS[site.relation_pid][locale]} · {ACCESS[site.visitability][locale]}
              </span>
              <span className="visit-heritage-list__title">{site.label}</span>
              <span className="visit-heritage-list__place">{site.wikidata_qid}</span>
            </button>
            <a target="_blank" rel="noopener noreferrer" href={`https://www.wikidata.org/wiki/${site.wikidata_qid}`}>
              {locale === "fr" ? "Voir la source Wikidata ↗" : "Wikidata source ↗"}
            </a>
          </li>
        ))}
      </ul>
    </section>
  );
}
