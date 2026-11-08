// web/src/features/poreia/components/poreia-catalog-sheet.tsx
import { PoreiaPlaceCard } from "@/features/poreia/components/poreia-place-card";
import type { HeritagePlace } from "@/features/poreia/heritage-catalog";
import { formatPoreiaDay } from "@/features/poreia/poreia-search";
import type { PoreiaCatalogSite, PoreiaOccurrenceItem, VisitSite } from "@/lib/entity-views";
import { useI18n } from "@/lib/i18n";

const KNOWN_PIDS = new Set(["P547", "P138", "P119", "P551"]);

export function catalogPlace(site: PoreiaCatalogSite): HeritagePlace {
  const link = site.historical_links[0];
  const relationPid = link?.relation_pid && KNOWN_PIDS.has(link.relation_pid)
    ? link.relation_pid as VisitSite["relation_pid"]
    : undefined;
  return {
    id: site.id,
    title: site.label,
    kind: site.kind,
    relationPid,
    coordinates: site.coordinates,
    visitability: site.visitability,
    summary: link?.summary ?? null,
    wikidataQid: site.wikidata_qid,
    openingHours: site.opening_hours,
    hoursCheckedAt: site.hours_checked_at,
    origins: ["wikidata_site"],
    sources: [
      { label: "Wikidata", url: site.source_url },
      ...site.historical_links.map((item) => ({ label: item.relation_role, url: item.source_url })),
    ],
  };
}

export function PoreiaCatalogSheet({
  sites,
  occurrences,
  selectedId,
  pinned,
  loading,
  hasMore,
  onSelect,
  onMore,
}: {
  sites: PoreiaCatalogSite[];
  occurrences: PoreiaOccurrenceItem[];
  selectedId?: string;
  pinned?: PoreiaCatalogSite;
  loading: boolean;
  hasMore: boolean;
  onSelect: (site: PoreiaCatalogSite) => void;
  onMore: () => void;
}) {
  const { locale, t } = useI18n();
  const selected = sites.find((site) => site.id === selectedId) ?? (pinned?.id === selectedId ? pinned : undefined);
  return (
    <section className="poreia-catalog" aria-labelledby="poreia-catalog-title">
      <h2 id="poreia-catalog-title">{t.catalogTitle}</h2>
      {loading && !sites.length ? <p role="status">{t.loading}</p> : null}
      {!loading && !sites.length ? <p role="status">{t.catalogEmpty}</p> : null}
      {selected ? <PoreiaPlaceCard place={catalogPlace(selected)} /> : null}
      <ul className="visit-heritage-list">
        {sites.map((site) => (
          <li key={site.id}>
            <button
              type="button"
              aria-current={site.id === selectedId ? "true" : undefined}
              onClick={() => onSelect(site)}
            >
              {site.label}
            </button>
          </li>
        ))}
      </ul>
      {hasMore ? (
        <button type="button" className="button button--ghost" onClick={onMore}>{t.loadMorePlaces}</button>
      ) : null}
      <h2>{t.agendaTitle}</h2>
      <p className="v3-note">{t.agendaNote}</p>
      {occurrences.length ? (
        <ul className="visit-now-list">
          {occurrences.map((item) => (
            <li key={item.id}>
              <span>{item.kind.replaceAll("_", " ")}</span>
              <strong>{item.title}</strong>
              {item.occurs_on ? <span>{formatPoreiaDay(item.occurs_on, locale)}</span> : null}
              {item.ends_on ? <span>{formatPoreiaDay(item.ends_on, locale)}</span> : null}
              <a href={item.source_url} target="_blank" rel="noopener noreferrer">{t.visitNowOpenSource}</a>
            </li>
          ))}
        </ul>
      ) : (
        <p role="status">{t.agendaEmpty}</p>
      )}
    </section>
  );
}
