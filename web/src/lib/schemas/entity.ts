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
  profiles?: Array<{
    slug: string;
    label: string;
    kind: string;
    qid?: string | null;
    confidence?: number;
    source_system?: string;
  }>;
}

export interface PeriodFacet {
  id: string;
  slug: string;
  label: string;
  start_year?: number | null;
  end_year?: number | null;
  kind: string;
}

export interface ProfileFacet {
  slug: string;
  label: string;
  entity_count: number;
}
