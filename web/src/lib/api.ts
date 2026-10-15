// web/src/lib/api.ts
import type { EntityProfile, SearchSuggestion } from "@/lib/schemas/entity";

export interface EventTime {
  kind: "exact" | "range" | "approx" | "unknown";
  start?: string | null;
  end?: string | null;
  precision?: "day" | "month" | "year";
  calendar?: string;
  surface?: string | null;
}

export interface TimelineEvent {
  id: string;
  entity_id: string;
  person: string;
  event_type: string;
  epistemic_status: string;
  title: string;
  summary?: string | null;
  time?: EventTime;
  start_time?: string | null;
  place_label?: string | null;
  confidence?: number;
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
    claims?: number;
  };
  llm?: {
    configured: boolean;
    model: string;
  };
  catalogs?: {
    openalex: boolean;
    europeana: boolean;
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
  /** Active person-ingest events (default). Pass `quality` or `legacy` to inspect older rows. */
  pipeline?: "person" | "quality" | "legacy";
  limit?: number;
  lang?: string;
}

function timelineSearchParams(query: TimelineQuery = {}): URLSearchParams {
  const params = new URLSearchParams({
    limit: String(query.limit ?? 2000),
    pipeline: query.pipeline ?? "person",
  });
  if (query.entityId) params.set("entity_id", query.entityId);
  if (query.person?.trim()) params.set("person", query.person.trim());
  if (query.profileSlug) params.set("profile_slug", query.profileSlug);
  if (query.periodSlug) params.set("period_slug", query.periodSlug);
  if (query.lang) params.set("lang", query.lang);
  return params;
}

export async function fetchTimeline(query: TimelineQuery = {}): Promise<TimelineResponse> {
  const response = await fetch(`/api/v1/timeline?${timelineSearchParams(query)}`);
  if (!response.ok) throw new Error("timeline fetch failed");
  return response.json();
}

export async function fetchGeoJson(query: TimelineQuery = {}): Promise<GeoJsonFeatureCollection> {
  const response = await fetch(`/api/v1/events/geojson?${timelineSearchParams(query)}`);
  if (!response.ok) throw new Error("geojson fetch failed");
  return response.json();
}

export async function fetchStatus(): Promise<StatusResponse> {
  const response = await fetch("/api/v1/status");
  if (!response.ok) throw new Error("status fetch failed");
  return response.json();
}

export async function searchEntities(
  query: string,
  lang = "en",
): Promise<SearchSuggestion[]> {
  const params = new URLSearchParams({
    q: query.trim(),
    limit: "10",
    lang: lang === "fr" ? "fr" : "en",
  });
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

export async function fetchEventDetail(
  eventId: string,
  lang?: string,
): Promise<EventDetailResponse> {
  const query = lang ? `?lang=${encodeURIComponent(lang)}` : "";
  const response = await fetch(`/api/v1/events/${eventId}${query}`);
  if (!response.ok) throw new Error("event detail fetch failed");
  return response.json();
}

export interface ClaimEvidence {
  id: string;
  source_system: string;
  locator?: string | null;
  quote?: string | null;
  sentence_id?: string | null;
  confidence: number;
  document_id?: string | null;
  document_title?: string | null;
  document_url?: string | null;
  document_type?: string | null;
  source_kind?: string | null;
}

export interface EntityClaim {
  id: string;
  claim_kind: string;
  text: string;
  epistemic_status: string;
  relation_to_subject: string;
  confidence: number;
  canonical_event_id?: string | null;
  debate_type?: string | null;
  evidence_layer?: string | null;
  evidence: ClaimEvidence[];
}

export async function fetchEntityClaims(
  entityId: string,
  opts: { limit?: number; debatesOnly?: boolean; lang?: string } = {},
): Promise<EntityClaim[]> {
  const params = new URLSearchParams({
    limit: String(opts.limit ?? 50),
    debates_only: String(opts.debatesOnly ?? true),
  });
  if (opts.lang) params.set("lang", opts.lang);
  const response = await fetch(`/api/v1/entities/${entityId}/claims?${params}`);
  if (!response.ok) throw new Error("claims fetch failed");
  const data = (await response.json()) as { claims?: EntityClaim[] };
  return data.claims ?? [];
}

export interface BibliographyLink {
  relation: string;
  score: number;
  match_version?: string | null;
  evidence_summary?: string | null;
}

export interface BibliographyItem {
  id: string;
  title: string;
  document_type: string;
  source_kind: string;
  external_id: string;
  canonical_url?: string | null;
  academic_status: string;
  language?: string | null;
  publication_time?: unknown;
  epistemic: string;
  link: BibliographyLink;
}

export interface BibliographyResponse {
  providers?: { name: string; count: number }[];
  entity_id: string;
  relation: string;
  epistemic: string;
  epistemic_note: string;
  items: BibliographyItem[];
  next_cursor?: string | null;
}

export async function fetchEntityBibliography(
  entityId: string,
  opts: { limit?: number; relation?: string; providers?: string; cursor?: string; signal?: AbortSignal } = {},
): Promise<BibliographyResponse> {
  const params = new URLSearchParams({
    limit: String(opts.limit ?? 40),
    relation: opts.relation ?? "about",
  });
  if (opts.providers) params.set("providers", opts.providers);
  if (opts.cursor) params.set("cursor", opts.cursor);
  const response = await fetch(`/api/v1/entities/${entityId}/bibliography?${params}`, { signal: opts.signal });
  if (!response.ok) throw new Error("bibliography fetch failed");
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

export type IngestLane = "explorer" | "agora";

export interface IngestJobResponse {
  job_id: string;
  lane?: IngestLane | string;
  purpose?: string | null;
  status: "queued" | "running" | "done" | "failed" | string;
  subject: string;
  qid?: string | null;
  entity_id?: string | null;
  timeline_events?: number;
  map_events?: number;
  error?: string | null;
  report?: unknown;
  deduped?: boolean;
}

export function browserWikiLang(): string {
  const lang = (navigator.language || "en").slice(0, 2).toLowerCase();
  return /^[a-z]{2}$/.test(lang) ? lang : "en";
}

export const EXPLORER_INGEST_MAX_DOCUMENTS = 400;

async function startLaneIngest(
  lane: IngestLane,
  input: {
    subject: string;
    qid?: string | null;
    live?: boolean;
    maxTitles?: number;
    maxDocuments?: number;
    wikiLang?: string;
    corpusLimit?: number;
    corpusLimitPerProvider?: number;
    corpusLimitTotal?: number;
    minimumPerProvider?: number;
  },
): Promise<IngestJobResponse> {
  const response = await fetch(`/api/v1/ingest/${lane}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      subject: input.subject,
      qid: input.qid ?? undefined,
      live: input.live ?? true,
      max_titles: input.maxTitles,
      max_documents: input.maxDocuments,
      wiki_lang: input.wikiLang,
      corpus_limit: input.corpusLimit,
      corpus_limit_per_provider: input.corpusLimitPerProvider,
      corpus_limit_total: input.corpusLimitTotal,
      minimum_per_provider: input.minimumPerProvider,
    }),
  });
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: string } | null;
    throw new Error(body?.error ?? `${lane} ingest start failed`);
  }
  return response.json();
}

/** Quality life-trace: dated events with places for map + timeline. */
export async function startExplorerIngest(input: {
  subject: string;
  qid?: string | null;
  live?: boolean;
  maxTitles?: number;
  maxDocuments?: number;
  wikiLang?: string;
}): Promise<IngestJobResponse> {
  return startLaneIngest("explorer", {
    ...input,
    maxDocuments: input.maxDocuments ?? EXPLORER_INGEST_MAX_DOCUMENTS,
    wikiLang: input.wikiLang ?? browserWikiLang(),
  });
}

/** Historiography layer: corpus sources + soft claims (debates, theories). */
export async function startAgoraIngest(input: {
  subject: string;
  qid?: string | null;
  live?: boolean;
  corpusLimit?: number;
  corpusLimitPerProvider?: number;
  corpusLimitTotal?: number;
  minimumPerProvider?: number;
}): Promise<IngestJobResponse> {
  return startLaneIngest("agora", input);
}

/** @deprecated Use startExplorerIngest */
export async function startPersonIngest(input: {
  subject: string;
  qid?: string | null;
  live?: boolean;
}): Promise<IngestJobResponse> {
  return startExplorerIngest(input);
}

export async function fetchIngestJob(jobId: string): Promise<IngestJobResponse> {
  const response = await fetch(`/api/v1/ingest/${jobId}`);
  if (!response.ok) throw new Error("ingest job fetch failed");
  return response.json();
}

export interface ExplorerIngestStatus {
  job_id: string;
  status: "queued" | "running" | "done" | "failed" | string;
  phase: string;
  current_page?: string | null;
  entity_id?: string | null;
  timeline_events: number;
  map_pins: number;
  precise_dates: number;
  precise_coords: number;
  evidence_count: number;
  wiki_pages: number;
  wdqs_events: number;
  sources_pending?: number;
  elapsed_ms?: number | null;
  is_done: boolean;
  error?: string | null;
}

export async function fetchExplorerStatus(jobId: string): Promise<ExplorerIngestStatus> {
  const response = await fetch(`/api/v1/ingest/explorer/${jobId}/status`);
  if (!response.ok) throw new Error("explorer status fetch failed");
  return response.json();
}

export interface DemoRosterItem {
  qid: string;
  entity_id?: string | null;
  label?: string | null;
  event_count: number;
  map_pin_count: number;
  claim_count: number;
  intuition_count: number;
  known_locally: boolean;
}

export interface DemoRosterResponse {
  items: DemoRosterItem[];
  count: number;
  intuition?: { network?: string; live_allowed?: boolean };
}

export async function fetchDemoRoster(): Promise<DemoRosterResponse> {
  const response = await fetch("/api/v1/demo/roster");
  if (!response.ok) throw new Error("demo roster fetch failed");
  return response.json();
}

export interface TalariaAuthUser {
  id: string;
  wallet_address: string | null;
}

export interface TalariaMeResponse {
  authenticated: boolean;
  user?: TalariaAuthUser;
}

export async function fetchTalariaMe(): Promise<TalariaMeResponse> {
  const response = await fetch("/api/v1/auth/me", { credentials: "include" });
  if (!response.ok) throw new Error("auth me failed");
  return response.json();
}

export async function requestWalletChallenge(
  address: string,
  chainId: number,
): Promise<{ challenge_id: string; message: string }> {
  const response = await fetch("/api/v1/auth/wallet/challenge", {
    method: "POST",
    credentials: "include",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ address, chain_id: chainId }),
  });
  const data = await response.json();
  if (!response.ok) {
    throw new Error(data?.error?.code ?? "challenge_failed");
  }
  return data;
}

export async function verifyWalletChallenge(
  challengeId: string,
  message: string,
  signature: string,
): Promise<{ user: TalariaAuthUser }> {
  const response = await fetch("/api/v1/auth/wallet/verify", {
    method: "POST",
    credentials: "include",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      challenge_id: challengeId,
      message,
      signature,
    }),
  });
  const data = await response.json();
  if (!response.ok) {
    throw new Error(data?.error?.code ?? "verify_failed");
  }
  return data;
}

export async function logoutTalariaSession(): Promise<void> {
  await fetch("/api/v1/auth/logout", { method: "POST", credentials: "include" });
}

export type InteractionAction =
  | "interest"
  | "follow"
  | "save"
  | "support"
  | "dispute"
  | "uncertain"
  | "useful"
  | "credible"
  | "not_credible"
  | "want_to_visit"
  | "visited";

export type InteractionTargetType =
  | "person"
  | "claim"
  | "source"
  | "place"
  | "event";

export type InteractionVisibility = "private" | "public";

export interface Interaction {
  id: string;
  action: InteractionAction;
  target_type: InteractionTargetType;
  target_id: string;
  visibility: InteractionVisibility;
  created_at: string;
  updated_at: string;
}

export interface InteractionListResponse {
  items: Interaction[];
  next_cursor: string | null;
}

export interface InteractionSummary {
  target_id: string;
  counts: Record<string, number>;
  comment_count?: number;
  argument_count?: number;
  source_count?: number;
  mine: InteractionAction[];
}

export type CommentReactionType =
  | "relevant"
  | "well_sourced"
  | "interesting"
  | "needs_nuance"
  | "disagree";

export interface CommentAuthor {
  id: string;
  display_name: string | null;
}

export interface AgoraComment {
  id: string;
  claim_id: string;
  author: CommentAuthor;
  body: string | null;
  status: "active" | "deleted";
  edited_at: string | null;
  created_at: string;
  reactions: Record<string, number>;
  my_reactions: CommentReactionType[];
  replies?: AgoraComment[];
  reply_count?: number;
}

export async function fetchClaimComments(
  claimId: string,
  params: { cursor?: string; limit?: number } = {},
): Promise<{ items: AgoraComment[]; next_cursor: string | null }> {
  const qs = new URLSearchParams();
  if (params.cursor) qs.set("cursor", params.cursor);
  if (params.limit) qs.set("limit", String(params.limit));
  const suffix = qs.size ? `?${qs}` : "";
  const response = await fetch(
    `/api/v1/claims/${encodeURIComponent(claimId)}/comments${suffix}`,
    { credentials: "include" },
  );
  if (!response.ok) await readApiError(response);
  return response.json();
}

export async function postClaimComment(
  claimId: string,
  body: string,
  parentCommentId?: string | null,
): Promise<{ comment: AgoraComment }> {
  const response = await fetch(`/api/v1/claims/${encodeURIComponent(claimId)}/comments`, {
    method: "POST",
    credentials: "include",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ body, parent_comment_id: parentCommentId ?? null }),
  });
  if (!response.ok) await readApiError(response);
  return response.json();
}

export type ArgumentRelation = "supports" | "contradicts" | "qualifies";

export interface ClaimSource {
  corpus_document_id: string;
  title: string;
  source_kind: string;
  canonical_url?: string | null;
  document_type: string;
  created_at: string;
}

export interface AgoraArgument {
  id: string;
  target_claim_id: string;
  relation: ArgumentRelation;
  statement: string;
  origin: string;
  contribution_status: string;
  created_at: string;
  author?: { id: string; display_name?: string | null } | null;
  sources: ClaimSource[];
  evidence: {
    id: string;
    quote?: string | null;
    locator?: string | null;
    source_system: string;
  }[];
}

export async function fetchClaimSources(claimId: string): Promise<{ items: ClaimSource[] }> {
  const response = await fetch(`/api/v1/claims/${encodeURIComponent(claimId)}/sources`, {
    credentials: "include",
  });
  if (!response.ok) await readApiError(response);
  return response.json();
}

export async function postClaimSource(
  claimId: string,
  corpusDocumentId: string,
): Promise<{ source: ClaimSource }> {
  const response = await fetch(`/api/v1/claims/${encodeURIComponent(claimId)}/sources`, {
    method: "POST",
    credentials: "include",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ corpus_document_id: corpusDocumentId }),
  });
  if (!response.ok) await readApiError(response);
  return response.json();
}

export async function fetchClaimArguments(
  claimId: string,
): Promise<{ items: AgoraArgument[] }> {
  const response = await fetch(`/api/v1/claims/${encodeURIComponent(claimId)}/arguments`, {
    credentials: "include",
  });
  if (!response.ok) await readApiError(response);
  return response.json();
}

export async function postClaimArgument(
  claimId: string,
  body: {
    relation: ArgumentRelation;
    statement: string;
    corpus_document_id: string;
    quote: string;
  },
): Promise<{ argument: AgoraArgument }> {
  const response = await fetch(`/api/v1/claims/${encodeURIComponent(claimId)}/arguments`, {
    method: "POST",
    credentials: "include",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!response.ok) await readApiError(response);
  return response.json();
}

export async function promoteCommentToArgument(
  commentId: string,
  body: {
    relation: ArgumentRelation;
    statement?: string;
    corpus_document_id: string;
    quote: string;
  },
): Promise<{ argument: AgoraArgument }> {
  const response = await fetch(`/api/v1/comments/${encodeURIComponent(commentId)}/promote`, {
    method: "POST",
    credentials: "include",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!response.ok) await readApiError(response);
  return response.json();
}

export async function patchComment(
  commentId: string,
  body: string,
): Promise<{ comment: AgoraComment }> {
  const response = await fetch(`/api/v1/comments/${encodeURIComponent(commentId)}`, {
    method: "PATCH",
    credentials: "include",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ body }),
  });
  if (!response.ok) await readApiError(response);
  return response.json();
}

export async function deleteComment(commentId: string): Promise<{ comment: AgoraComment }> {
  const response = await fetch(`/api/v1/comments/${encodeURIComponent(commentId)}`, {
    method: "DELETE",
    credentials: "include",
  });
  if (!response.ok) await readApiError(response);
  return response.json();
}

export async function addCommentReaction(
  commentId: string,
  reactionType: CommentReactionType,
): Promise<{ comment: AgoraComment }> {
  const response = await fetch(`/api/v1/comments/${encodeURIComponent(commentId)}/reactions`, {
    method: "POST",
    credentials: "include",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ reaction_type: reactionType }),
  });
  if (!response.ok) await readApiError(response);
  return response.json();
}

export async function removeCommentReaction(
  commentId: string,
  reactionType: CommentReactionType,
): Promise<{ comment: AgoraComment }> {
  const response = await fetch(
    `/api/v1/comments/${encodeURIComponent(commentId)}/reactions/${encodeURIComponent(reactionType)}`,
    { method: "DELETE", credentials: "include" },
  );
  if (!response.ok) await readApiError(response);
  return response.json();
}

export class ApiError extends Error {
  code: string;
  constructor(code: string, message?: string) {
    super(message ?? code);
    this.code = code;
  }
}

async function readApiError(response: Response): Promise<never> {
  const data = await response.json().catch(() => ({}));
  throw new ApiError(data?.error?.code ?? "request_failed", data?.error?.message);
}

export async function postInteraction(body: {
  action: InteractionAction;
  target_type: InteractionTargetType;
  target_id: string;
  visibility?: InteractionVisibility;
}): Promise<{ interaction: Interaction; created: boolean }> {
  const response = await fetch("/api/v1/interactions", {
    method: "POST",
    credentials: "include",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!response.ok) await readApiError(response);
  const data = (await response.json()) as { interaction: Interaction };
  return { interaction: data.interaction, created: response.status === 201 };
}

export async function deleteInteraction(id: string): Promise<void> {
  const response = await fetch(`/api/v1/interactions/${encodeURIComponent(id)}`, {
    method: "DELETE",
    credentials: "include",
  });
  if (!response.ok) await readApiError(response);
}

export async function fetchMyInteractions(params: {
  action?: InteractionAction;
  target_type?: InteractionTargetType;
  cursor?: string;
  limit?: number;
} = {}): Promise<InteractionListResponse> {
  const qs = new URLSearchParams();
  if (params.action) qs.set("action", params.action);
  if (params.target_type) qs.set("target_type", params.target_type);
  if (params.cursor) qs.set("cursor", params.cursor);
  if (params.limit) qs.set("limit", String(params.limit));
  const suffix = qs.size ? `?${qs}` : "";
  const response = await fetch(`/api/v1/me/interactions${suffix}`, {
    credentials: "include",
  });
  if (!response.ok) await readApiError(response);
  return response.json();
}

export type ClaimIntuitionStatus =
  | {
      status: "ready";
      claim_id: string;
      chain_id: number;
      triple_term_id: string;
      counter_term_id: string | null;
    }
  | { status: "not_published"; claim_id: string };

export async function fetchClaimIntuition(claimId: string): Promise<ClaimIntuitionStatus> {
  const response = await fetch(`/api/v1/claims/${encodeURIComponent(claimId)}/intuition`, {
    credentials: "include",
  });
  if (!response.ok) await readApiError(response);
  return response.json();
}

export async function syncClaimSignal(
  claimId: string,
  action: "support" | "dispute",
  txHash: string,
): Promise<{ interaction: Interaction; synced: boolean }> {
  const response = await fetch(`/api/v1/claims/${encodeURIComponent(claimId)}/signals/sync`, {
    method: "POST",
    credentials: "include",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ action, tx_hash: txHash }),
  });
  if (!response.ok) await readApiError(response);
  return response.json();
}

export async function fetchInteractionSummary(
  targetType: InteractionTargetType,
  targetIds: string[],
): Promise<{ summaries: InteractionSummary[] }> {
  if (targetIds.length > 100) {
    throw new ApiError("batch_too_large");
  }
  const qs = new URLSearchParams({
    target_type: targetType,
    target_ids: targetIds.join(","),
  });
  const response = await fetch(`/api/v1/interactions/summary?${qs}`, {
    credentials: "include",
  });
  if (!response.ok) await readApiError(response);
  return response.json();
}
