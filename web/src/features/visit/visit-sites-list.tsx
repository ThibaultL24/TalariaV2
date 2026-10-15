import type { VisitSite } from "@/lib/entity-views";

const RELATION: Record<VisitSite["relation_pid"], string> = {
  P547: "Hommage", P138: "Nommé en l’honneur de", P119: "Sépulture", P551: "Résidence",
};

export function VisitSitesList({
  sites, onFocus,
}: {
  sites: VisitSite[];
  onFocus: (site: VisitSite) => void;
}) {
  if (!sites.length) return null;
  return (
    <section className="visit-wikidata-sites" aria-label="Sites patrimoniaux documentés">
      <h2>Sites patrimoniaux Wikidata</h2>
      <ul className="visit-heritage-list">
        {sites.map((site) => (
          <li key={`${site.id}:${site.relation_pid}`}>
            <button type="button" onClick={() => onFocus(site)}>
              <span className="visit-heritage-list__meta">
                {RELATION[site.relation_pid]} · {site.visitability === "unknown" ? "Visite non vérifiée" : site.visitability}
              </span>
              <span className="visit-heritage-list__title">{site.label}</span>
              <span className="visit-heritage-list__place">{site.wikidata_qid}</span>
            </button>
            <a target="_blank" rel="noopener noreferrer" href={`https://www.wikidata.org/wiki/${site.wikidata_qid}`}>
              Source Wikidata ↗
            </a>
          </li>
        ))}
      </ul>
    </section>
  );
}
