// web/src/features/poreia/heritage-catalog.ts
import type { TimelineEvent } from "@/lib/api";
import type { VisitSite } from "@/lib/entity-views";

export type HeritageOrigin = "wikidata_site" | "heritage_event" | "current_activity";

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
  hoursCheckedAt?: string;
}

function recordedHours(hours?: string | null, checkedAt?: string | null): { openingHours?: string; hoursCheckedAt?: string } {
  const openingHours = hours?.trim();
  const hoursCheckedAt = checkedAt?.trim();
  if (!openingHours || !hoursCheckedAt) return {};
  return { openingHours, hoursCheckedAt };
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
    ...recordedHours(site.opening_hours, site.hours_checked_at),
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

interface MapReadingFeature {
  id?: string | number;
  geometry?: { type?: string; coordinates?: unknown };
  properties?: Record<string, unknown>;
}

function textProp(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function pointCoordinates(feature: MapReadingFeature): { lat: number; lon: number } | undefined {
  if (feature.geometry?.type !== "Point" || !Array.isArray(feature.geometry.coordinates)) return undefined;
  const [lon, lat] = feature.geometry.coordinates;
  if (typeof lat !== "number" || typeof lon !== "number" || !Number.isFinite(lat) || !Number.isFinite(lon)) return undefined;
  return { lat, lon };
}

/**
 * The map list reads the same features the map draws. Identity is the feature id.
 * A second feature with the same id is ignored. Coordinates never create a match.
 */
export function placesFromMapFeatures(features: MapReadingFeature[]): HeritagePlace[] {
  const places: HeritagePlace[] = [];
  const seen = new Set<string>();
  for (const feature of features) {
    const props = feature.properties ?? {};
    const id = textProp(props.id) ?? (feature.id == null ? undefined : String(feature.id));
    const title = textProp(props.title);
    if (!id || !title || seen.has(id)) continue;
    seen.add(id);
    const layer = textProp(props.visit_layer);
    const origin: HeritageOrigin = layer === "heritage_site"
      ? "wikidata_site"
      : layer === "now"
        ? "current_activity"
        : "heritage_event";
    const qid = textProp(props.wikidata_qid);
    const relation = textProp(props.relation_pid);
    const visitability = textProp(props.visitability);
    const sourceUrl = textProp(props.association_source_url) ?? textProp(props.canonical_url);
    places.push({
      id,
      title,
      kind: textProp(props.kind) ?? textProp(props.event_type) ?? "place",
      relationPid: relation === "P547" || relation === "P138" || relation === "P119" || relation === "P551" ? relation : undefined,
      city: textProp(props.place_label) ?? textProp(props.venue_label),
      coordinates: pointCoordinates(feature),
      visitability: visitability === "verified_visitable" || visitability === "verified_closed" || visitability === "restricted"
        ? visitability
        : "unknown",
      sources: sourceUrl ? [{ label: origin === "wikidata_site" ? "Wikidata" : "Source", url: sourceUrl }] : [],
      origins: [origin],
      wikidataQid: qid && /^Q\d+$/.test(qid) ? qid : undefined,
      summary: textProp(props.summary),
      ...recordedHours(textProp(props.opening_hours), textProp(props.hours_checked_at)),
    });
  }
  return places;
}

function readEventQid(event: TimelineEvent): string | undefined {
  const raw = (event as TimelineEvent & { wikidata_qid?: string }).wikidata_qid;
  return raw && /^Q\d+$/.test(raw) ? raw : undefined;
}
