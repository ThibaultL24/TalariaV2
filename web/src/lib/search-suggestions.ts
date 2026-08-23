// web/src/lib/search-suggestions.ts
import type { SearchSuggestion } from "@/lib/schemas/entity";

const UUID_RE =
  /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i;

/** Dev/test ingest rows — not a person the user should pick. */
export function isPersonSearchNoise(label: string): boolean {
  const trimmed = label.trim();
  if (trimmed.length < 2) return true;
  if (/LotD/i.test(trimmed)) return true;
  if (UUID_RE.test(trimmed)) return true;
  return false;
}

function namesOverlap(query: string, label: string): boolean {
  const q = query.trim().toLowerCase();
  const l = label.trim().toLowerCase();
  if (!q || !l) return false;
  return l.includes(q) || q.includes(l);
}

/** One row: the typed name, backed by the densest matching entity when we have one. */
export function collapseToSinglePersonSuggestion(
  query: string,
  items: SearchSuggestion[],
): SearchSuggestion[] {
  const label = query.trim();
  if (label.length < 2) return [];

  const usable = items.filter((item) => !isPersonSearchNoise(item.label));
  const localMatches = usable
    .filter((item) => item.known_locally && item.entity_id)
    .sort((a, b) => (b.event_count ?? 0) - (a.event_count ?? 0));

  const bestLocal =
    localMatches.find((item) => namesOverlap(label, item.label)) ?? localMatches[0];

  if (bestLocal) {
    return [
      {
        ...bestLocal,
        label,
        description: bestLocal.description ?? bestLocal.label,
      },
    ];
  }

  const remote =
    usable.find((item) => !item.known_locally && namesOverlap(label, item.label)) ??
    usable.find((item) => !item.known_locally);

  if (remote) {
    return [{ ...remote, label, description: remote.description ?? remote.label }];
  }

  return [
    {
      label,
      known_locally: false,
      entity_id: null,
      qid: null,
      description: null,
      event_count: 0,
    },
  ];
}
