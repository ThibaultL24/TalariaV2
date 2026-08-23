// web/src/components/search/entity-search-box.tsx
import { useCallback, useEffect, useRef, useState } from "react";
import type { SearchSuggestion } from "@/lib/schemas/entity";
import { useI18n } from "@/lib/i18n";

interface EntitySearchBoxProps {
  suggestions: SearchSuggestion[];
  onSubmitQuery: (trimmedQuery: string) => void;
  onSelect: (item: SearchSuggestion) => void;
  isLoading?: boolean;
}

export function EntitySearchBox({
  suggestions,
  onSubmitQuery,
  onSelect,
  isLoading,
}: EntitySearchBoxProps) {
  const { t } = useI18n();
  const [value, setValue] = useState("");
  const [isFocused, setIsFocused] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const blurCloseTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearBlurTimer = useCallback(() => {
    if (blurCloseTimerRef.current) {
      clearTimeout(blurCloseTimerRef.current);
      blurCloseTimerRef.current = null;
    }
  }, []);

  useEffect(() => () => {
    clearBlurTimer();
    if (debounceRef.current) clearTimeout(debounceRef.current);
  }, [clearBlurTimer]);

  const trimmed = value.trim();
  const showPanel = isFocused && trimmed.length >= 2;
  const showList = suggestions.length > 0 && !isLoading;

  function queueSearch(next: string) {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      onSubmitQuery(next.trim());
    }, 220);
  }

  return (
    <div className="relative">
      <input
        type="search"
        value={value}
        onChange={(event) => {
          const next = event.target.value;
          setValue(next);
          queueSearch(next);
        }}
        onFocus={() => {
          clearBlurTimer();
          setIsFocused(true);
          if (trimmed.length >= 2) onSubmitQuery(trimmed);
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
          }
        }}
        placeholder={t.searchPlaceholder}
        className="person-filter min-w-0 w-full"
        aria-label={t.personSearch}
        aria-autocomplete="list"
        aria-expanded={showPanel}
        autoComplete="off"
      />

      {showPanel ? (
        <div
          className="absolute top-full right-0 left-0 z-30 mt-1.5 overflow-hidden rounded-xl border border-(--color-border-subtle) bg-(--color-bg-elevated) shadow-lg"
          role="listbox"
        >
          <div className="border-b border-(--color-border-subtle) px-3 py-2 text-[11px] text-(--color-text-muted)">
            {t.searchHint}
          </div>
          {isLoading && !showList ? (
            <div className="px-3 py-3 text-sm text-(--color-text-secondary)">{t.loading}</div>
          ) : null}
          {!isLoading && suggestions.length === 0 ? (
            <div className="px-3 py-3 text-sm text-(--color-text-secondary)">{t.noResults}</div>
          ) : null}
          {showList
            ? suggestions.map((item) => (
                <button
                  key={`${item.qid ?? "local"}-${item.entity_id ?? item.label}`}
                  type="button"
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => {
                    clearBlurTimer();
                    onSelect(item);
                    setValue(item.label);
                    onSubmitQuery("");
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
                    {item.qid ? (
                      <span className="shrink-0 text-[10px] text-(--color-text-muted)">{item.qid}</span>
                    ) : null}
                  </div>
                </button>
              ))
            : null}
        </div>
      ) : null}
    </div>
  );
}
