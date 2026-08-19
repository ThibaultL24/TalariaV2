// web/src/components/search/entity-search-box.tsx
import { useCallback, useEffect, useRef, useState } from "react";
import type { SearchSuggestion } from "@/lib/schemas/entity";
import { strings } from "@/lib/strings";

interface EntitySearchBoxProps {
  suggestions: SearchSuggestion[];
  onSubmitQuery: (trimmedQuery: string) => void;
  onSelect: (item: SearchSuggestion) => void;
  isLoading?: boolean;
  placeholder?: string;
}

export function EntitySearchBox({
  suggestions,
  onSubmitQuery,
  onSelect,
  isLoading,
  placeholder = strings.searchPlaceholder,
}: EntitySearchBoxProps) {
  const [value, setValue] = useState("");
  const [lastSubmitted, setLastSubmitted] = useState("");
  const [isFocused, setIsFocused] = useState(false);
  const blurCloseTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearBlurTimer = useCallback(() => {
    if (blurCloseTimerRef.current) {
      clearTimeout(blurCloseTimerRef.current);
      blurCloseTimerRef.current = null;
    }
  }, []);

  useEffect(() => () => clearBlurTimer(), [clearBlurTimer]);

  const trimmed = value.trim();
  const hasMinQuery = trimmed.length >= 2;
  const hasSubmittedCurrentQuery = lastSubmitted === trimmed && hasMinQuery;
  const showSuggestionPanel = isFocused && hasSubmittedCurrentQuery;
  const showSuggestionsList = suggestions.length > 0 && !isLoading;

  function submitQuery() {
    if (!hasMinQuery) return;
    setLastSubmitted(trimmed);
    onSubmitQuery(trimmed);
  }

  function clearSubmitted() {
    setLastSubmitted("");
    onSubmitQuery("");
  }

  return (
    <div className="relative">
      <div className="flex gap-2">
        <input
          type="search"
          value={value}
          onChange={(event) => setValue(event.target.value)}
          onFocus={() => {
            clearBlurTimer();
            setIsFocused(true);
          }}
          onBlur={() => {
            clearBlurTimer();
            blurCloseTimerRef.current = setTimeout(() => {
              setIsFocused(false);
              blurCloseTimerRef.current = null;
            }, 150);
          }}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              event.preventDefault();
              (event.target as HTMLInputElement).blur();
              return;
            }
            if (event.key !== "Enter") return;
            event.preventDefault();
            submitQuery();
          }}
          placeholder={placeholder ?? strings.searchPlaceholder}
          className="person-filter min-w-0 flex-1"
          aria-label="Entity search"
          aria-autocomplete="list"
          aria-expanded={showSuggestionPanel}
          autoComplete="off"
        />
        <button
          type="button"
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => {
            clearBlurTimer();
            submitQuery();
            setIsFocused(true);
          }}
          disabled={!hasMinQuery}
          className="shrink-0 rounded-xl border border-(--color-border-subtle) bg-(--color-bg-surface) px-3 py-2 text-sm font-medium disabled:opacity-40"
        >
          {strings.search}
        </button>
      </div>

      {showSuggestionPanel ? (
        <div
          className="absolute top-full right-0 left-0 z-20 mt-1.5 overflow-hidden rounded-xl border border-(--color-border-subtle) bg-(--color-bg-elevated) shadow-lg"
          role="listbox"
        >
          <div className="border-b border-(--color-border-subtle) px-3 py-2 text-[11px] text-(--color-text-muted)">
            {strings.searchHintCommit}
          </div>
          {isLoading && !showSuggestionsList ? (
            <div className="px-3 py-3 text-sm text-(--color-text-secondary)">{strings.loading}</div>
          ) : null}
          {!isLoading && suggestions.length === 0 ? (
            <div className="px-3 py-3 text-sm text-(--color-text-secondary)">{strings.noResults}</div>
          ) : null}
          {showSuggestionsList
            ? suggestions.map((item) => (
                <button
                  key={`${item.qid ?? "local"}-${item.entity_id ?? item.label}`}
                  type="button"
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => {
                    clearBlurTimer();
                    onSelect(item);
                    setValue(item.label);
                    clearSubmitted();
                    setIsFocused(false);
                  }}
                  className="block w-full px-3 py-2 text-left hover:bg-(--color-bg-primary)"
                  role="option"
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0 flex-1">
                      <div className="font-medium">{item.label}</div>
                      {item.description ? (
                        <div className="text-xs opacity-70">{item.description}</div>
                      ) : null}
                    </div>
                    <span className="shrink-0 text-[10px]">
                      {item.known_locally ? (
                        <span className="rounded bg-emerald-500/15 px-1.5 py-0.5 text-emerald-400">
                          {strings.searchInLibrary}
                          {item.event_count != null ? ` · ${item.event_count}` : ""}
                        </span>
                      ) : (
                        <span className="rounded bg-sky-500/15 px-1.5 py-0.5 text-sky-300">
                          {strings.searchNew}
                          {item.qid ? ` · ${item.qid}` : ""}
                        </span>
                      )}
                    </span>
                  </div>
                </button>
              ))
            : null}
        </div>
      ) : null}
    </div>
  );
}
