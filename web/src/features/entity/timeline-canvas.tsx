import { useEffect, useMemo, useRef, useState, type CSSProperties, type PointerEvent as ReactPointerEvent } from "react";
import type { TimelineEvent } from "@/lib/api";
import { eventDate } from "@/lib/entity-views";
import { LEGEND_COLORS, legendKeyForEventType } from "@/lib/event-legend";
import { clampWindow, windowCenteredOn } from "./timeline-window";

function yearOf(value?: string | null): number | null {
  const match = value?.match(/^(-?\d{4,})(?:-|$)/);
  return match ? Number(match[1]) : null;
}

function eventYear(event: TimelineEvent): number | null {
  if (event.time?.kind === "unknown") return null;
  return yearOf(event.time?.start);
}

// Keep the horizontal date exact; stack neighbours vertically so every target is reachable.
export function layoutDots(events: TimelineEvent[], from: number, to: number, width: number) {
  const rows: number[][] = [];
  return events.flatMap((event) => {
    const start = eventYear(event);
    const end = yearOf(event.time?.end) ?? start;
    if (start === null || start > to || (end ?? start) < from) return [];
    const x = 22 + ((Math.max(from, start) - from) / Math.max(1, to - from)) * Math.max(1, width - 44);
    let row = rows.findIndex((xs) => xs.every((other) => Math.abs(other - x) >= 24));
    if (row < 0) {
      row = rows.length;
      rows.push([]);
    }
    rows[row].push(x);
    return [{ event, x, row }];
  });
}

export function TimelineCanvas({
  events,
  from,
  to,
  lifeFrom,
  lifeTo,
  onSelect,
  onZoom,
}: {
  events: TimelineEvent[];
  from: number;
  to: number;
  lifeFrom: number;
  lifeTo: number;
  onSelect: (id: string) => void;
  onZoom: (from: number, to: number) => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const trackRef = useRef<HTMLDivElement>(null);
  const drag = useRef<{ mode: "start" | "end" | "move"; originX: number; from: number; to: number } | null>(null);
  const ignoreScroll = useRef(false);
  const movedByList = useRef(false);
  const [width, setWidth] = useState(800);
  const [hovered, setHovered] = useState<TimelineEvent>();

  useEffect(() => {
    const node = host.current;
    if (!node) return;
    const observer = new ResizeObserver(() => setWidth(node.clientWidth));
    observer.observe(node);
    setWidth(node.clientWidth);
    return () => observer.disconnect();
  }, []);

  const [windowFrom, windowTo] = clampWindow(lifeFrom, lifeTo, from, to);
  const dots = useMemo(
    () => layoutDots(events, windowFrom, windowTo, width),
    [events, windowFrom, windowTo, width],
  );
  const height = dots.reduce((max, dot) => Math.max(max, 80 + dot.row * 24), 320);
  const preview = hovered && events.find((event) => event.id === hovered.id);
  const undated = events.filter((event) => eventYear(event) === null);
  const listed = useMemo(
    () =>
      events
        .filter((event) => {
          const year = eventYear(event);
          return year !== null && year >= windowFrom && year <= windowTo;
        })
        .sort((a, b) => (eventYear(a) ?? 0) - (eventYear(b) ?? 0)),
    [events, windowFrom, windowTo],
  );
  const span = Math.max(1, lifeTo - lifeFrom);
  const brushLeft = ((windowFrom - lifeFrom) / span) * 100;
  const brushWidth = (Math.max(1, windowTo - windowFrom) / span) * 100;

  useEffect(() => {
    if (movedByList.current) {
      movedByList.current = false;
      return;
    }
    const list = listRef.current;
    if (!list) return;
    ignoreScroll.current = true;
    list.scrollTop = 0;
    const frame = window.requestAnimationFrame(() => {
      ignoreScroll.current = false;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [windowFrom, windowTo]);

  function zoomTo(nextFrom: number, nextTo: number) {
    const [a, b] = clampWindow(lifeFrom, lifeTo, nextFrom, nextTo);
    if (a !== windowFrom || b !== windowTo) onZoom(a, b);
  }

  function onListScroll() {
    if (ignoreScroll.current) return;
    const list = listRef.current;
    if (!list) return;
    const edge = list.getBoundingClientRect().top + 8;
    const rows = [...list.querySelectorAll<HTMLElement>("[data-year]")];
    const current = rows.find((row) => row.getBoundingClientRect().bottom > edge) ?? rows[0];
    const year = Number(current?.dataset.year);
    if (!Number.isFinite(year)) return;
    const [a, b] = windowCenteredOn(lifeFrom, lifeTo, windowTo - windowFrom, year);
    if (a === windowFrom && b === windowTo) return;
    movedByList.current = true;
    onZoom(a, b);
  }

  function yearAt(clientX: number): number {
    const rect = trackRef.current?.getBoundingClientRect();
    if (!rect || rect.width <= 0) return lifeFrom;
    const ratio = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
    return lifeFrom + ratio * span;
  }

  function onBrushPointerDown(event: ReactPointerEvent<HTMLElement>, mode: "start" | "end" | "move") {
    drag.current = { mode, originX: event.clientX, from: windowFrom, to: windowTo };
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function onBrushPointerMove(event: ReactPointerEvent<HTMLElement>) {
    const state = drag.current;
    const rect = trackRef.current?.getBoundingClientRect();
    if (!state || !rect || rect.width <= 0) return;
    const deltaYears = ((event.clientX - state.originX) / rect.width) * span;
    if (state.mode === "move") {
      zoomTo(state.from + deltaYears, state.to + deltaYears);
      return;
    }
    const year = yearAt(event.clientX);
    if (state.mode === "start") zoomTo(Math.min(year, state.to - 1), state.to);
    else zoomTo(state.from, Math.max(year, state.from + 1));
  }

  function onBrushPointerUp() {
    drag.current = null;
  }

  return (
    <section className="v3-constellation" aria-label="Interactive timeline">
      <div className="v3-timeline-toolbar">
        <div>
          <span className="v3-eyebrow">A LIFE IN MOMENTS</span>
          <h2>
            {windowFrom} — {windowTo}
          </h2>
        </div>
      </div>
      <p className="v3-note">
        {dots.length} moments · Hover a point or scroll the list. Drag the bar to choose the years.
      </p>
      <div className="v3-life-grid">
        <div
          ref={listRef}
          className="v3-life-list"
          onScroll={onListScroll}
          aria-label="Moments in this period"
        >
          {listed.map((event) => (
            <button
              key={event.id}
              type="button"
              data-year={eventYear(event) ?? undefined}
              data-event-id={event.id}
              onClick={() => onSelect(event.id)}
            >
              <span>{eventDate(event)}</span>
              {event.title}
            </button>
          ))}
        </div>
        <div>
          <div className="v3-dot-scroll">
            <div ref={host} className="v3-dot-field" style={{ height }}>
              {Array.from({ length: 6 }, (_, i) => (
                <div className="v3-year-guide" key={i} style={{ left: `${3 + i * 18.8}%` }}>
                  <span>{Math.round(windowFrom + ((windowTo - windowFrom) * i) / 5)}</span>
                </div>
              ))}
              <div key={`${windowFrom}-${windowTo}`} className="v3-dot-fall">
              {dots.map(({ event, x, row }) => {
                const top = height - 48 - row * 24;
                return (
                <button
                  key={event.id}
                  type="button"
                  className={`v3-dot ${event.time?.kind === "approx" ? "v3-dot--approx" : ""}`}
                  style={
                    {
                      left: x,
                      top,
                      "--dot-color": LEGEND_COLORS[legendKeyForEventType(event.event_type)],
                      "--dot-delay": `${Math.min(Math.round(top / 28) * 45, 900)}ms`,
                    } as CSSProperties
                  }
                  aria-label={`${eventDate(event)} · ${event.title}`}
                  title={`${eventDate(event)} · ${event.title}`}
                  onMouseEnter={() => setHovered(event)}
                  onFocus={() => setHovered(event)}
                  onClick={() => onSelect(event.id)}
                >
                  <span />
                </button>
              );})}
              </div>
            </div>
          </div>
          <div className="v3-dot-preview" aria-live="polite">
            {preview ? (
              <>
                <span>{eventDate(preview)}</span>
                <button type="button" onClick={() => onSelect(preview.id)}>
                  {preview.title} ↗
                </button>
              </>
            ) : (
              <span>Select a point to explore its story.</span>
            )}
          </div>
          <div
            ref={trackRef}
            className="v3-brush"
            aria-label="Life span"
          >
            {events.map((event) => {
              const year = eventYear(event);
              if (year === null) return null;
              const left = ((year - lifeFrom) / span) * 100;
              return <i key={event.id} className="v3-brush-dot" style={{ left: `${left}%` }} />;
            })}
            <div
              className="v3-brush-window"
              style={{ left: `${brushLeft}%`, width: `${brushWidth}%` }}
              onPointerDown={(event) => onBrushPointerDown(event, "move")}
              onPointerMove={onBrushPointerMove}
              onPointerUp={onBrushPointerUp}
            >
              <button
                type="button"
                className="v3-brush-handle v3-brush-handle--start"
                aria-label="Earlier edge"
                onPointerDown={(event) => {
                  event.stopPropagation();
                  onBrushPointerDown(event, "start");
                }}
                onPointerMove={onBrushPointerMove}
                onPointerUp={onBrushPointerUp}
              />
              <button
                type="button"
                className="v3-brush-handle v3-brush-handle--end"
                aria-label="Later edge"
                onPointerDown={(event) => {
                  event.stopPropagation();
                  onBrushPointerDown(event, "end");
                }}
                onPointerMove={onBrushPointerMove}
                onPointerUp={onBrushPointerUp}
              />
            </div>
          </div>
        </div>
      </div>
      {!dots.length && <p>No dated events in this period.</p>}
      {!!undated.length && (
        <div className="v3-undated">
          <span>Date not established</span>
          {undated.map((event) => (
            <button
              key={event.id}
              type="button"
              onClick={() => onSelect(event.id)}
              aria-label={event.title}
              title={event.title}
            >
              ●
            </button>
          ))}
        </div>
      )}
      <p className="v3-note">
        One point per sourced occurrence. Hollow points indicate approximate dates.
      </p>
    </section>
  );
}
