// web/src/lib/api.ts
import type { EntityProfile, SearchSuggestion } from "@/lib/schemas/entity";

export interface TimelineEvent {
  id: string;
  entity_id: string;
  person: string;
  event_type: string;
  epistemic_status: string;
  title: string;
  summary?: string | null;
  start_time?: string | null;
  place_label?: string | null;
  confidence: number;
  map_eligible: boolean;
  coordinates?: { lat: number; lon: number } | null;
}

export interface TimelineResponse {
  count: number;
  events: TimelineEvent[];
}

export interface GeoJsonFeatureCollection {
  type: "FeatureCollection";
  features: GeoJsonFeature[];
}

export interface GeoJsonFeature {
  type: "Feature";
  id?: string;
  geometry: { type: "Point"; coordinates: [number, number] };
  properties: Record<string, unknown>;
}

export interface StatusResponse {
  offline_only?: boolean;
  counts: {
    wiki_pages: number;
    sentences: number;
    phrase_candidates: number;
    canonical_events: number;
    entity_profiles?: number;
  };
}

export interface EventEvidence {
  id: string;
  quoted_text?: string | null;
  sentence_text?: string | null;
  confidence: number;
  wiki_title?: string | null;
  wiki_lang?: string | null;
  revision_id?: number | null;
  sentence_ordinal?: number | null;
  page_url?: string | null;
  revision_url?: string | null;
  citation_url?: string | null;
}

export interface EventSourceRef {
  type?: string;
  kind?: string;
  source_system?: string | null;
  language?: string | null;
  page_title?: string | null;
  source_page_title?: string | null;
  oldid?: number | null;
  revision_id?: number | null;
  snippet?: string | null;
  quote?: string | null;
  label?: string | null;
  section_title?: string | null;
  sentence_ordinal?: number | null;
  offset_start?: number | null;
  offset_end?: number | null;
  url?: string | null;
  source_url?: string | null;
  wikipedia_url?: string | null;
  page_url?: string | null;
  revision_url?: string | null;
  confidence?: number;
  evidence_id?: string | null;
  citation_index?: number | null;
  inline_citations?: string[] | null;
}

export interface EventDetailResponse {
  event: TimelineEvent | null;
  entity?: {
    id: string;
    label: string;
    wikipedia_title?: string;
    qid?: string | null;
  } | null;
  links?: {
    wikipedia_url?: string | null;
    wikipedia_revision_url?: string | null;
    wikidata_url?: string | null;
  };
  narrative?: {
    event_summary?: string | null;
    how_it_happened?: string | null;
    fact?: string | null;
    context_note?: string | null;
    context_sentences?: Array<{
      text: string;
      is_evidence: boolean;
      ordinal: number;
    }>;
    summary?: string | null;
  };
  source_refs?: EventSourceRef[];
  source_page_titles?: string[];
  narrative_sentences?: Array<{
    id: string;
    ordinal: number;
    text: string;
    is_evidence: boolean;
  }>;
  evidence?: EventEvidence[];
}

export interface TimelineQuery {
  entityId?: string;
  person?: string;
  profileSlug?: string;
  periodSlug?: string;
  limit?: number;
}

export async function fetchTimeline(query: TimelineQuery = {}): Promise<TimelineResponse> {
  const params = new URLSearchParams({ limit: String(query.limit ?? 500) });
  if (query.entityId) params.set("entity_id", query.entityId);
  if (query.person?.trim()) params.set("person", query.person.trim());
  if (query.profileSlug) params.set("profile_slug", query.profileSlug);
  if (query.periodSlug) params.set("period_slug", query.periodSlug);
  const response = await fetch(`/api/v1/timeline?${params}`);
  if (!response.ok) throw new Error("timeline fetch failed");
  return response.json();
}

export async function fetchGeoJson(query: TimelineQuery = {}): Promise<GeoJsonFeatureCollection> {
  const params = new URLSearchParams({ limit: String(query.limit ?? 500) });
  if (query.entityId) params.set("entity_id", query.entityId);
  if (query.person?.trim()) params.set("person", query.person.trim());
  if (query.profileSlug) params.set("profile_slug", query.profileSlug);
  if (query.periodSlug) params.set("period_slug", query.periodSlug);
  const response = await fetch(`/api/v1/events/geojson?${params}`);
  if (!response.ok) throw new Error("geojson fetch failed");
  return response.json();
}

export async function fetchStatus(): Promise<StatusResponse> {
  const response = await fetch("/api/v1/status");
  if (!response.ok) throw new Error("status fetch failed");
  return response.json();
}

export async function searchEntities(query: string): Promise<SearchSuggestion[]> {
  const params = new URLSearchParams({ q: query.trim(), limit: "10" });
  const response = await fetch(`/api/v1/entities/search?${params}`);
  if (!response.ok) throw new Error("entity search failed");
  const data = (await response.json()) as { items: SearchSuggestion[] };
  return data.items ?? [];
}

export async function fetchEntity(entityId: string): Promise<EntityProfile | null> {
  const response = await fetch(`/api/v1/entities/${entityId}`);
  if (!response.ok) throw new Error("entity fetch failed");
  const data = (await response.json()) as { entity: EntityProfile | null };
  return data.entity;
}

export async function fetchEventEvidence(eventId: string): Promise<EventEvidence[]> {
  const response = await fetch(`/api/v1/events/${eventId}/evidence`);
  if (!response.ok) throw new Error("evidence fetch failed");
  const data = (await response.json()) as { evidence: EventEvidence[] };
  return data.evidence ?? [];
}

export async function fetchEventDetail(eventId: string): Promise<EventDetailResponse> {
  const response = await fetch(`/api/v1/events/${eventId}`);
  if (!response.ok) throw new Error("event detail fetch failed");
  return response.json();
}

export async function fetchPeriods(): Promise<import("@/lib/schemas/entity").PeriodFacet[]> {
  const response = await fetch("/api/v1/periods");
  if (!response.ok) throw new Error("periods fetch failed");
  const data = (await response.json()) as { periods: import("@/lib/schemas/entity").PeriodFacet[] };
  return data.periods ?? [];
}

export async function fetchProfiles(): Promise<import("@/lib/schemas/entity").ProfileFacet[]> {
  const response = await fetch("/api/v1/profiles");
  if (!response.ok) throw new Error("profiles fetch failed");
  const data = (await response.json()) as { profiles: import("@/lib/schemas/entity").ProfileFacet[] };
  return data.profiles ?? [];
}
