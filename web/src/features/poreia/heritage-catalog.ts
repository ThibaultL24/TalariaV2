// web/src/features/poreia/heritage-catalog.ts
import type { TimelineEvent } from "@/lib/api";
import type { VisitSite } from "@/lib/entity-views";

export type HeritageOrigin = "wikidata_site" | "heritage_event";

export interface HeritageSource {
  label: string;
  url?: string;
}

/** One row in the tourist reading list. Origins stay separate. */
export interface HeritagePlace {
  id: string;
  title: string;
  kind: string;
  relationPid?: VisitSite["relation_pid"];
  city?: string | null;
  coordinates?: { lat: number; lon: number };
  visitability: VisitSite["visitability"];
  sources: HeritageSource[];
  origins: HeritageOrigin[];
  wikidataQid?: string;
  summary?: string | null;
  openingHours?: string;
}

function sitePlace(site: VisitSite): HeritagePlace {
  return {
    id: site.id,
    title: site.label,
    kind: site.kind,
    relationPid: site.relation_pid,
    coordinates: site.coordinates,
    visitability: site.visitability,
    wikidataQid: site.wikidata_qid,
    openingHours: site.opening_hours?.trim() || undefined,
    sources: [{
      label: "Wikidata",
      url: site.association_source_url || `https://www.wikidata.org/wiki/${site.wikidata_qid}`,
    }],
    origins: ["wikidata_site"],
  };
}

function eventPlace(event: TimelineEvent): HeritagePlace {
  return {
    id: event.id,
    title: event.title,
    kind: event.event_type,
    city: event.place_label,
    coordinates: event.coordinates ?? undefined,
    visitability: "unknown",
    summary: event.summary,
    sources: [],
    origins: ["heritage_event"],
  };
}

/**
 * Keep every sourced place. Two rows collapse only when they share an id or a
 * Wikidata QID. Matching coordinates is not enough.
 */
export function buildHeritageCatalog(sites: VisitSite[], events: TimelineEvent[]): HeritagePlace[] {
  const places = sites.map(sitePlace);
  const seenIds = new Set(places.map((place) => place.id));
  const seenQids = new Set(places.flatMap((place) => place.wikidataQid ? [place.wikidataQid] : []));

  for (const event of events) {
    if (seenIds.has(event.id)) continue;
    const qid = readEventQid(event);
    if (qid && seenQids.has(qid)) {
      const match = places.find((place) => place.wikidataQid === qid);
      if (!match) continue;
      if (!match.origins.includes("heritage_event")) match.origins.push("heritage_event");
      if (!match.city && event.place_label) match.city = event.place_label;
      if (!match.summary && event.summary) match.summary = event.summary;
      continue;
    }
    places.push(eventPlace(event));
    seenIds.add(event.id);
  }
  return places;
}

function readEventQid(event: TimelineEvent): string | undefined {
  const raw = (event as TimelineEvent & { wikidata_qid?: string }).wikidata_qid;
  return raw && /^Q\d+$/.test(raw) ? raw : undefined;
}
