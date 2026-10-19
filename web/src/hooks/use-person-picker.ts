// web/src/hooks/use-person-picker.ts
import { useCallback, useEffect, useState } from "react";
import { searchEntities } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import type { SearchSuggestion } from "@/lib/schemas/entity";
import { rankPersonSuggestions } from "@/lib/search-suggestions";
import { useExplorerStore } from "@/stores/explorer-store";
import { useProgressiveIngest } from "./use-progressive-ingest";

const SEARCH_DELAY_MS = 180;
const SEARCH_TTL_MS = 30_000;
const CACHE_LIMIT = 100;
const suggestionCache = new Map<string, { expiry: number; items: SearchSuggestion[] }>();

function getCached(query: string, locale: string): SearchSuggestion[] | undefined {
  const key = `${locale}:${query.toLowerCase()}`;
  const hit = suggestionCache.get(key);
  if (hit && hit.expiry > Date.now()) return hit.items;
  suggestionCache.delete(key);
  return undefined;
}

function cacheSuggestions(query: string, locale: string, items: SearchSuggestion[]) {
  const key = `${locale}:${query.toLowerCase()}`;
  suggestionCache.delete(key);
  suggestionCache.set(key, { expiry: Date.now() + SEARCH_TTL_MS, items });
  if (suggestionCache.size > CACHE_LIMIT) {
    suggestionCache.delete(suggestionCache.keys().next().value!);
  }
}

export interface UsePersonPickerOptions {
  startLifeIngest?: boolean;
  onCountsChanged?: (timeline: number, mapPins: number) => void;
}

export function usePersonPicker(opts: UsePersonPickerOptions = {}) {
  const startLifeIngest = opts.startLifeIngest ?? true;
  const { locale } = useI18n();
  const { setEntity, setPersonFilter } = useExplorerStore();
  const [searchQuery, setSearchQuery] = useState("");
  const [suggestions, setSuggestions] = useState<SearchSuggestion[]>([]);
  const [searchLoading, setSearchLoading] = useState(false);

  const progressiveIngest = useProgressiveIngest({
    onCountsChanged: opts.onCountsChanged,
    onEntityResolved: (id) => {
      const current = useExplorerStore.getState();
      if (current.entityId !== id) setEntity(id, current.entityLabel, current.entityQid);
    },
  });

  useEffect(() => {
    const q = searchQuery.trim();
    if (q.length < 2) {
      setSuggestions([]);
      setSearchLoading(false);
      return;
    }
    const controller = new AbortController();
    const cached = getCached(q, locale);
    if (cached) {
      setSuggestions(cached);
      setSearchLoading(false);
      return () => controller.abort();
    }
    setSearchLoading(true);
    const timer = window.setTimeout(() => {
      void searchEntities(q, locale, controller.signal)
        .then((items) => {
          if (controller.signal.aborted) return;
          const ranked = rankPersonSuggestions(q, items);
          cacheSuggestions(q, locale, ranked);
          setSuggestions(ranked);
        })
        .catch(() => {
          if (!controller.signal.aborted) setSuggestions([]);
        })
        .finally(() => {
          if (!controller.signal.aborted) setSearchLoading(false);
        });
    }, SEARCH_DELAY_MS);
    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [searchQuery, locale]);

  const selectPerson = useCallback(
    (item: SearchSuggestion): string | null => {
      // Preserve the exact identity selected by the user, including homonyms.
      const chosen = item;
      const localId =
        chosen.known_locally && chosen.entity_id ? chosen.entity_id : null;

      if (localId) {
        setEntity(localId, chosen.label, chosen.qid);
      } else {
        setPersonFilter(chosen.label, chosen.label, chosen.qid);
      }

      if (startLifeIngest) {
        void progressiveIngest
          .startIngest({
            subject: chosen.label,
            qid: chosen.qid,
            wikiLang: locale,
          })
          .then((job) => {
            if (job?.entity_id) {
              setEntity(job.entity_id, chosen.label, chosen.qid);
            }
          });
      }
      return localId;
    },
    [locale, setEntity, setPersonFilter, startLifeIngest, progressiveIngest],
  );

  return {
    searchQuery,
    setSearchQuery,
    suggestions,
    searchLoading,
    selectPerson,
    jobId: progressiveIngest.jobId,
    ingestBusy: progressiveIngest.isRunning,
    error: progressiveIngest.error,
    // Progressive ingest state
    ingestPhase: progressiveIngest.phase,
    currentPage: progressiveIngest.currentPage,
    timelineEvents: progressiveIngest.timelineEvents,
    mapPins: progressiveIngest.mapPins,
    preciseDates: progressiveIngest.preciseDates,
    preciseCoords: progressiveIngest.preciseCoords,
    evidenceCount: progressiveIngest.evidenceCount,
    wikiPages: progressiveIngest.wikiPages,
    wdqsEvents: progressiveIngest.wdqsEvents,
    sourcesPending: progressiveIngest.sourcesPending,
    elapsedMs: progressiveIngest.elapsedMs,
  };
}
