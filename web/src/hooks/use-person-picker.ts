// web/src/hooks/use-person-picker.ts
import { useCallback, useEffect, useState } from "react";
import { searchEntities } from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import { runExplorerPersonIngest } from "@/lib/person-ingest";
import type { SearchSuggestion } from "@/lib/schemas/entity";
import { collapseToSinglePersonSuggestion } from "@/lib/search-suggestions";
import { useExplorerStore } from "@/stores/explorer-store";

function namesOverlap(left: string, right: string): boolean {
  const a = left.trim().toLowerCase();
  const b = right.trim().toLowerCase();
  return a.includes(b) || b.includes(a);
}

function preferDenseLocalAlias(
  item: SearchSuggestion,
  items: SearchSuggestion[],
): SearchSuggestion {
  if (!item.known_locally || !item.label) return item;
  const denser = items
    .filter(
      (row) =>
        row.known_locally &&
        row.entity_id &&
        row.label &&
        namesOverlap(row.label, item.label) &&
        (row.event_count ?? 0) > (item.event_count ?? 0),
    )
    .sort((a, b) => (b.event_count ?? 0) - (a.event_count ?? 0))[0];
  return denser ?? item;
}

export function usePersonPicker(opts: { startLifeIngest?: boolean } = {}) {
  const startLifeIngest = opts.startLifeIngest ?? true;
  const { locale, t } = useI18n();
  const { setEntity, setPersonFilter } = useExplorerStore();
  const [searchQuery, setSearchQuery] = useState("");
  const [suggestions, setSuggestions] = useState<SearchSuggestion[]>([]);
  const [searchLoading, setSearchLoading] = useState(false);
  const [ingestBusy, setIngestBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!searchQuery.trim()) {
      setSuggestions([]);
      return;
    }
    let cancelled = false;
    setSearchLoading(true);
    searchEntities(searchQuery, locale)
      .then((items) => {
        if (!cancelled) setSuggestions(collapseToSinglePersonSuggestion(searchQuery, items));
      })
      .catch(() => {
        if (!cancelled) setSuggestions([]);
      })
      .finally(() => {
        if (!cancelled) setSearchLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [searchQuery, locale]);

  const selectPerson = useCallback(
    (item: SearchSuggestion) => {
      const chosen = preferDenseLocalAlias(item, suggestions);
      setError(null);
      if (chosen.known_locally && chosen.entity_id) {
        setEntity(chosen.entity_id, chosen.label, chosen.qid);
      } else {
        setPersonFilter(chosen.label, chosen.label, chosen.qid);
      }
      if (!startLifeIngest) return;
      setIngestBusy(true);
      void runExplorerPersonIngest({
        subject: chosen.label,
        qid: chosen.qid,
        entityId: chosen.entity_id,
        wikiLang: locale,
        onEntity: (id) => setEntity(id, chosen.label, chosen.qid),
      })
        .then((result) => {
          if (result?.status === "failed") setError(result.error ?? t.loadingMap);
        })
        .catch((err) => {
          setError(err instanceof Error ? err.message : t.loadingMap);
        })
        .finally(() => setIngestBusy(false));
    },
    [locale, setEntity, setPersonFilter, startLifeIngest, suggestions, t.loadingMap],
  );

  return {
    searchQuery,
    setSearchQuery,
    suggestions,
    searchLoading,
    selectPerson,
    ingestBusy,
    error,
  };
}
