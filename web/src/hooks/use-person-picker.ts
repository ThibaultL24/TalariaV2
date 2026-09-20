// web/src/hooks/use-person-picker.ts
import { useCallback, useEffect, useState } from "react";
import { searchEntities } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import type { SearchSuggestion } from "@/lib/schemas/entity";
import { collapseToSinglePersonSuggestion } from "@/lib/search-suggestions";
import { useExplorerStore } from "@/stores/explorer-store";
import { useProgressiveIngest } from "./use-progressive-ingest";

function namesOverlap(left: string, right: string): boolean {
  const fold = (value: string) =>
    value.normalize("NFD").replace(/\p{M}/gu, "").trim().toLowerCase();
  const a = fold(left);
  const b = fold(right);
  return a.includes(b) || b.includes(a);
}

function preferDenseLocalAlias(
  item: SearchSuggestion,
  items: SearchSuggestion[],
): SearchSuggestion {
  const denser = items
    .filter(
      (row) =>
        row.known_locally &&
        row.entity_id &&
        row.label &&
        (item.qid ? row.qid === item.qid : namesOverlap(row.label, item.label ?? "")) &&
        (row.event_count ?? 0) >= (item.event_count ?? 0),
    )
    .sort((a, b) => (b.event_count ?? 0) - (a.event_count ?? 0))[0];
  return denser ?? item;
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
  });

  useEffect(() => {
    const q = searchQuery.trim();
    if (!q) {
      setSuggestions([]);
      setSearchLoading(false);
      return;
    }
    let cancelled = false;
    setSearchLoading(true);
    const timer = window.setTimeout(() => {
      searchEntities(q, locale)
        .then((items) => {
          if (!cancelled) setSuggestions(collapseToSinglePersonSuggestion(q, items));
        })
        .catch(() => {
          if (!cancelled) setSuggestions([]);
        })
        .finally(() => {
          if (!cancelled) setSearchLoading(false);
        });
    }, 220);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [searchQuery, locale]);

  const selectPerson = useCallback(
    (item: SearchSuggestion) => {
      const chosen = preferDenseLocalAlias(item, suggestions);
      
      if (chosen.known_locally && chosen.entity_id) {
        setEntity(chosen.entity_id, chosen.label, chosen.qid);
      } else {
        setPersonFilter(chosen.label, chosen.label, chosen.qid);
      }

      if (!startLifeIngest) return;

      progressiveIngest.startIngest({
        subject: chosen.label,
        qid: chosen.qid,
        wikiLang: locale,
      }).then((job) => {
        if (job?.entity_id) {
          setEntity(job.entity_id, chosen.label, chosen.qid);
        }
      });
    },
    [locale, setEntity, setPersonFilter, startLifeIngest, suggestions, progressiveIngest],
  );

  return {
    searchQuery,
    setSearchQuery,
    suggestions,
    searchLoading,
    selectPerson,
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
