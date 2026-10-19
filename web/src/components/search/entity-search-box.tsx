// web/src/components/search/entity-search-box.tsx
import { useCallback, useEffect, useRef, useState } from "react";
import type { SearchSuggestion } from "@/lib/schemas/entity";
import { PUBLIC_PERSON_SEARCH_ENABLED } from "@/lib/demo-mode";
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
  const searchEnabled = PUBLIC_PERSON_SEARCH_ENABLED;
  const [value, setValue] = useState("");
  const [isFocused, setIsFocused] = useState(false);
  const [activeIndex, setActiveIndex] = useState(-1);
  const listId = "person-search-options";
  const blurCloseTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearBlurTimer = useCallback(() => {
    if (blurCloseTimerRef.current) {
      clearTimeout(blurCloseTimerRef.current);
      blurCloseTimerRef.current = null;
    }
  }, []);

  useEffect(() => () => {
    clearBlurTimer();

  }, [clearBlurTimer]);

  const trimmed = value.trim();
  const showPanel = searchEnabled && isFocused && trimmed.length >= 2;
  const showList = suggestions.length > 0 && !isLoading;

  function selectSuggestion(item: SearchSuggestion) {
    clearBlurTimer();
    onSelect(item);
    setValue(item.label);
    onSubmitQuery("");
    setActiveIndex(-1);
    setIsFocused(false);
  }

  useEffect(() => {
    setActiveIndex(-1);
  }, [suggestions]);

  return (
    <div className="relative">
      <input
        type="search"
        value={searchEnabled ? value : ""}
        disabled={!searchEnabled}
        onChange={(event) => {
          if (!searchEnabled) return;
          const next = event.target.value;
          setValue(next);
          setActiveIndex(-1);
          onSubmitQuery(next);
        }}
        onFocus={() => {
          if (!searchEnabled) return;
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
            setIsFocused(false);
            (event.target as HTMLInputElement).blur();
          } else if (showPanel && showList && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
            event.preventDefault();
            setActiveIndex((current) => event.key === "ArrowDown"
              ? (current + 1) % suggestions.length
              : (current + suggestions.length - 1) % suggestions.length);
          } else if (event.key === "Enter" && showPanel && showList && activeIndex >= 0) {
            event.preventDefault();
            selectSuggestion(suggestions[activeIndex]);
          }
        }}
        placeholder={searchEnabled ? t.searchPlaceholder : t.searchUnavailable}
        className="person-filter min-w-0 w-full disabled:cursor-not-allowed disabled:opacity-70"
        aria-label={t.personSearch}
        aria-disabled={!searchEnabled}
        role="combobox"
        aria-controls={listId}
        aria-autocomplete="list"
        aria-expanded={showPanel}
        aria-activedescendant={showPanel && activeIndex >= 0 ? `person-search-option-${activeIndex}` : undefined}
        autoComplete="off"
      />

      {showPanel ? (
        <div
          className="absolute top-full right-0 left-0 z-30 mt-1.5 overflow-hidden rounded-xl border border-(--color-border-strong) bg-(--color-bg-surface) shadow-lg"
          role="listbox"
          id={listId}
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
            ? suggestions.map((item, index) => (
                <button
                  key={`${item.qid ?? "local"}-${item.entity_id ?? item.label}`}
                  type="button"
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => selectSuggestion(item)}
                  onMouseEnter={() => setActiveIndex(index)}
                  id={`person-search-option-${index}`}
                  aria-selected={activeIndex === index}
                  className="block w-full px-3 py-2 text-left hover:bg-(--color-primary-soft)"
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
