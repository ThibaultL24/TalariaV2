import type { SearchSuggestion } from "@/lib/schemas/entity";

const UUID_RE = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i;
const MAX_SUGGESTIONS = 5;

export function isPersonSearchNoise(label: string): boolean {
  const trimmed = label.trim();
  return trimmed.length < 2 || /LotD/i.test(trimmed) || UUID_RE.test(trimmed);
}

function fold(value: string): string {
  return value.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase().trim().replace(/\s+/g, " ");
}

function isEventPhrase(label: string): boolean {
  return /\b(battle|retreat|army|attack|treaty|siege|campaign|plan of)\b/i.test(label) || label.length > 80;
}

function isNotAPersonRecord(item: SearchSuggestion): boolean {
  const label = item.label.trim();
  if (/^(avenue|rue|boulevard|chemin|quai|impasse|place|road|street)\b/i.test(label)) return true;
  const hasQid = Boolean(item.qid && /^Q\d+$/.test(item.qid));
  if (isEventPhrase(label) && !hasQid) return true;
  if (!hasQid && fold(label) === fold(item.description ?? label)) return true;
  return false;
}

function isRecordedPlaceOrName(description?: string | null): boolean {
  const text = (description ?? "").toLowerCase();
  return /human settlement|family name|given name|genus|higher education|city in|county seat|wikimedia|taxon|\bband\b/.test(text);
}

function relevance(query: string, label: string): number {
  const name = fold(label);
  if (name === query) return 0;
  if (name.startsWith(query)) return 1;
  if (name.includes(query)) return 2;
  if (query.includes(name)) return 3;
  return 4;
}

/** A known fiche opens at once. A selected QID may be researched. A bare name may not. */
export function researchDecision(item: SearchSuggestion): "open" | "offer" | "unresolved" {
  if (item.known_locally && item.entity_id) return "open";
  if (item.qid && /^Q\d+$/.test(item.qid)) return "offer";
  return "unresolved";
}

/** Preserve distinct Wikidata identities; never merge on label or event density. */
export function rankPersonSuggestions(
  query: string,
  items: SearchSuggestion[],
  limit = MAX_SUGGESTIONS,
): SearchSuggestion[] {
  const q = fold(query);
  if (q.length < 2) return [];
  const candidates = items
    .filter((item) => !isPersonSearchNoise(item.label))
    .filter((item) => !isNotAPersonRecord(item))
    .sort((a, b) =>
      Number(isRecordedPlaceOrName(a.description)) - Number(isRecordedPlaceOrName(b.description)) ||
      relevance(q, a.label) - relevance(q, b.label) ||
      Number(Boolean(b.known_locally)) - Number(Boolean(a.known_locally)) ||
      (b.event_count ?? 0) - (a.event_count ?? 0) ||
      a.label.localeCompare(b.label),
    );
  const seenQids = new Set<string>();
  const seenIds = new Set<string>();
  const out: SearchSuggestion[] = [];
  for (const item of candidates) {
    if ((item.qid && seenQids.has(item.qid)) ||
        (item.entity_id && seenIds.has(item.entity_id))) continue;
    if (item.qid) seenQids.add(item.qid);
    if (item.entity_id) seenIds.add(item.entity_id);
    out.push(item);
    if (out.length >= Math.max(1, Math.min(MAX_SUGGESTIONS, limit))) break;
  }
  // Retain the existing manual ingest path when neither database has the person.
  return out.length ? out : [{
    label: query.trim(), known_locally: false, entity_id: null,
    qid: null, description: null, event_count: 0,
  }];
}
