// web/src/lib/schemas/entity.ts
export interface SearchSuggestion {
  entity_id?: string | null;
  qid?: string | null;
  label: string;
  description?: string | null;
  known_locally: boolean;
  event_count?: number;
  wikipedia_title?: string | null;
}

export interface EntityProfile {
  id: string;
  qid?: string | null;
  label: string;
  wikipedia_title?: string | null;
  event_count: number;
}
