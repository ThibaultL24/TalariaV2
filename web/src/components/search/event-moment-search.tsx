// web/src/components/search/event-moment-search.tsx
import { useCallback, useEffect, useId, useRef, useState } from "react";
import type { TimelineEvent } from "@/lib/api";
import { eventDate } from "@/lib/entity-views";
import { searchEvents } from "@/lib/event-search";

interface EventMomentSearchProps {
  events: TimelineEvent[];
  onPick: (event: TimelineEvent) => void;
  loading?: boolean;
  placeholder?: string;
}

export function EventMomentSearch({
  events,
  onPick,
  loading,
  placeholder = "Search a moment (e.g. Austerlitz, coronation…)",
}: EventMomentSearchProps) {
  const [value, setValue] = useState("");
  const [open, setOpen] = useState(false);
  const listId = useId();
  const blurTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const trimmed = value.trim();
  const results = open && trimmed.length >= 2 ? searchEvents(events, trimmed, 12) : [];

  const clearBlurTimer = useCallback(() => {
    if (blurTimer.current) {
      clearTimeout(blurTimer.current);
      blurTimer.current = null;
    }
  }, []);

  useEffect(() => () => clearBlurTimer(), [clearBlurTimer]);

  function pick(event: TimelineEvent) {
    setValue(event.title);
    setOpen(false);
    onPick(event);
  }

  return (
    <div className="v3-event-search">
      <label className="v3-event-search__label" htmlFor={`${listId}-input`}>
        Find a moment
      </label>
      <div className="v3-event-search__field">
        <input
          id={`${listId}-input`}
          type="search"
          className="v3-event-search__input"
          value={value}
          placeholder={placeholder}
          autoComplete="off"
          role="combobox"
          aria-expanded={open && results.length > 0}
          aria-controls={results.length ? `${listId}-list` : undefined}
          onChange={(e) => {
            setValue(e.target.value);
            setOpen(true);
          }}
          onFocus={() => setOpen(true)}
          onBlur={() => {
            clearBlurTimer();
            blurTimer.current = setTimeout(() => setOpen(false), 160);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" && results[0]) {
              e.preventDefault();
              pick(results[0]);
            }
            if (e.key === "Escape") setOpen(false);
          }}
        />
        {loading && <span className="v3-event-search__hint" aria-live="polite">Loading moments…</span>}
      </div>
      {open && trimmed.length >= 2 && (
        <ul id={`${listId}-list`} className="v3-event-search__results" role="listbox">
          {results.length === 0 && !loading && (
            <li className="v3-event-search__empty">No moments match “{trimmed}” in loaded events.</li>
          )}
          {results.map((event) => (
            <li key={event.id}>
              <button
                type="button"
                role="option"
                className="v3-event-search__option"
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => pick(event)}
              >
                <span className="v3-event-search__option-title">{event.title}</span>
                <span className="v3-event-search__option-meta">
                  {eventDate(event)}
                  {event.place_label ? ` · ${event.place_label}` : ""}
                  {!event.map_eligible ? " · timeline only" : ""}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
