import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { TimelineEvent } from "@/lib/api";
import { eventDate } from "@/lib/entity-views";
import {
  LEGEND_COLORS,
  type LegendKey,
  legendKeyForEventType,
  legendLabel,
} from "@/lib/event-legend";
import { useI18n } from "@/lib/i18n";
import { HistographyBrush, lifePhasePresets } from "./timeline-histography-brush";

type WindowYears = [number, number];
function year(value?: string | null): number | null {
  const match = value?.match(/^(-?\d{4,})(?:-|$)/);
  return match ? Number(match[1]) : null;
}
export function clampWindow(window: WindowYears, bounds: WindowYears): WindowYears {
  const full = Math.max(1, bounds[1] - bounds[0]);
  const span = Math.min(full, Math.max(1, window[1] - window[0]));
  const start = Math.max(bounds[0], Math.min(bounds[1] - span, window[0]));
  return [start, start + span];
}
export function zoomWindow(window: WindowYears, factor: number, anchor: number, bounds: WindowYears): WindowYears {
  const pivot = window[0] + (window[1] - window[0]) * anchor;
  const span = Math.max(1, (window[1] - window[0]) * factor);
  return clampWindow([pivot - span * anchor, pivot + span * (1 - anchor)], bounds);
}
// Stable vertical packing across zoom levels. Horizontal position always follows typed time.
export function layoutDots(events: TimelineEvent[], from: number, to: number, width: number) {
  const rows: number[][] = [];
  return events.flatMap((event) => {
    const start = event.time?.kind === "unknown" ? null : year(event.time?.start);
    const end = year(event.time?.end) ?? start;
    if (start === null || start > to || (end ?? start) < from) return [];
    const x = 22 + (Math.max(from, start) - from) / Math.max(1, to - from) * Math.max(1, width - 44);
    let row = rows.findIndex((xs) => xs.every((other) => Math.abs(other - x) >= 24));
    if (row < 0) { row = rows.length; rows.push([]); }
    rows[row].push(x);
    return [{ event, x, row, year: start }];
  });
}

export function TimelineCanvas({ events, from, to, bounds, onSelect, onZoom }: {
  events: TimelineEvent[]; from: number; to: number; bounds?: WindowYears;
  onSelect: (id: string) => void; onZoom: (from: number, to: number) => void;
}) {
  const initialBounds = useRef<WindowYears>([from, Math.max(from + 1, to)]);
  const domain = bounds ?? initialBounds.current;
  const fullBounds: WindowYears = [domain[0], Math.max(domain[0] + 1, domain[1])];
  const host = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(800);
  const [target, setTarget] = useState<WindowYears>(() => clampWindow([from, to], fullBounds));
  const [camera, setCamera] = useState<WindowYears>(target);
  const cameraRef = useRef(camera);
  const targetRef = useRef(target);
  const [hovered, setHovered] = useState<TimelineEvent>();
  const [activeLegend, setActiveLegend] = useState<Set<LegendKey> | null>(null);
  const { locale } = useI18n();
  const [dragging, setDragging] = useState(false);
  const gesture = useRef<{ x: number; window: WindowYears; moved: boolean } | undefined>(undefined);
  const suppressClick = useRef(false);
  const commitTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const onZoomRef = useRef(onZoom);
  onZoomRef.current = onZoom;
  const commit = (window: WindowYears) => onZoomRef.current(Math.floor(window[0]), Math.ceil(window[1]));
  const move = (window: WindowYears, save = true) => {
    const next = clampWindow(window, fullBounds);
    targetRef.current = next;
    setTarget(next);
    clearTimeout(commitTimer.current);
    if (save) commitTimer.current = setTimeout(() => commit(next), 180);
  };
  const moveRef = useRef(move);
  moveRef.current = move;
  useEffect(() => {
    const next = clampWindow([from, to], [domain[0], Math.max(domain[0] + 1, domain[1])]);
    targetRef.current = next;
    setTarget(next);
  }, [from, to, domain[0], domain[1]]);
  useEffect(() => {
    const reduced = window.matchMedia?.("(prefers-reduced-motion: reduce)");
    let frame = 0;
    const animate = () => {
      const previous = cameraRef.current;
      const done = reduced?.matches || Math.abs(previous[0] - target[0]) + Math.abs(previous[1] - target[1]) < .002;
      const next: WindowYears = done ? target : [previous[0] + (target[0] - previous[0]) * .16, previous[1] + (target[1] - previous[1]) * .16];
      cameraRef.current = next;
      setCamera(next);
      if (!done) frame = requestAnimationFrame(animate);
    };
    frame = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(frame);
  }, [target]);
  useEffect(() => {
    const node = host.current;
    if (!node) return;
    const observer = new ResizeObserver(() => setWidth(node.clientWidth || 800));
    observer.observe(node);
    setWidth(node.clientWidth || 800);
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      const current = targetRef.current;
      if (Math.abs(e.deltaX) > Math.abs(e.deltaY) || e.shiftKey) {
        const shift = (e.deltaX || e.deltaY) / node.clientWidth * (current[1] - current[0]);
        moveRef.current([current[0] + shift, current[1] + shift]);
      } else {
        const anchor = Math.max(0, Math.min(1, (e.clientX - node.getBoundingClientRect().left) / node.clientWidth));
        moveRef.current(zoomWindow(current, Math.exp(Math.max(-100, Math.min(100, e.deltaY)) * .003), anchor, [domain[0], Math.max(domain[0] + 1, domain[1])]));
      }
    };
    node.addEventListener("wheel", wheel, { passive: false });
    return () => { observer.disconnect(); node.removeEventListener("wheel", wheel); clearTimeout(commitTimer.current); };
  }, [domain[0], domain[1]]);
  const visibleEvents = useMemo(() => {
    if (!activeLegend?.size) return events;
    return events.filter((event) => activeLegend.has(legendKeyForEventType(event.event_type)));
  }, [events, activeLegend]);
  const dots = useMemo(
    () => layoutDots(visibleEvents, fullBounds[0], fullBounds[1], width),
    [visibleEvents, fullBounds[0], fullBounds[1], width],
  );
  const height = Math.max(420, dots.reduce((max, dot) => Math.max(max, 220 + dot.row * 18), 420));
  const span = Math.max(1, camera[1] - camera[0]);
  const preview = hovered && visibleEvents.find((event) => event.id === hovered.id);
  const undated = visibleEvents.filter(
    (event) => event.time?.kind === "unknown" || year(event.time?.start) === null,
  );
  const histogram = useMemo(() => {
    const bins = Array<number>(64).fill(0);
    for (const dot of dots) {
      bins[
        Math.min(
          63,
          Math.max(
            0,
            Math.floor(((dot.year - fullBounds[0]) / (fullBounds[1] - fullBounds[0])) * 64),
          ),
        )
      ]++;
    }
    return bins;
  }, [dots, fullBounds[0], fullBounds[1]]);
  const legendCounts = useMemo(() => {
    const counts = new Map<LegendKey, number>();
    for (const event of events) {
      const key = legendKeyForEventType(event.event_type);
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return counts;
  }, [events]);
  const phases = useMemo(() => lifePhasePresets(fullBounds), [fullBounds[0], fullBounds[1]]);
  const navigate = (next: WindowYears) => {
    move(next, false);
    commit(clampWindow(next, fullBounds));
  };
  function toggleLegend(key: LegendKey) {
    setActiveLegend((current) => {
      if (!current) return new Set([key]);
      const next = new Set(current);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next.size ? next : null;
    });
  }
  return (
    <section
      className="v3-constellation v3-time-space v3-histography"
      aria-label="Interactive timeline"
    >
      <header className="v3-histo-header">
        <div>
          <span className="v3-histo-kicker">Histography-style life map</span>
          <h2 className="v3-histo-range">
            {Math.round(camera[0])}
            <span aria-hidden="true"> — </span>
            {Math.round(camera[1])}
          </h2>
        </div>
        <div className="v3-timeline-controls v3-histo-zoom">
          <button type="button" onClick={() => navigate(zoomWindow(target, 2, 0.5, fullBounds))} aria-label="Zoom out">
            −
          </button>
          <button type="button" onClick={() => navigate(zoomWindow(target, 0.5, 0.5, fullBounds))} aria-label="Zoom in">
            +
          </button>
          <button type="button" onClick={() => navigate(fullBounds)} aria-label="Reset period">
            ↺
          </button>
        </div>
      </header>
      <div className="v3-histo-body">
        <aside className="v3-histo-categories" aria-label="Event categories">
          <button
            type="button"
            className={activeLegend === null ? "is-active" : ""}
            onClick={() => setActiveLegend(null)}
          >
            All moments <span>{events.length}</span>
          </button>
          {Array.from(legendCounts.entries())
            .sort((a, b) => b[1] - a[1])
            .map(([key, count]) => (
              <button
                key={key}
                type="button"
                className={activeLegend?.has(key) ? "is-active" : ""}
                onClick={() => toggleLegend(key)}
              >
                <span className="v3-histo-swatch" style={{ background: LEGEND_COLORS[key] }} />
                {legendLabel(key, locale)} <span>{count}</span>
              </button>
            ))}
        </aside>
        <div className="v3-dot-scroll v3-histo-canvas-wrap">
          <div className="v3-histo-axis" aria-hidden="true" />
          <div
            ref={host}
            className={`v3-dot-field v3-histo-field ${dragging ? "is-dragging" : ""}`}
            style={{ height }}
        onPointerDown={e => { if (e.button !== 0) return; suppressClick.current = false; gesture.current = { x: e.clientX, window: targetRef.current, moved: false }; }}
        onPointerMove={e => {
          const g = gesture.current;
          if (!g) return;
          const delta = e.clientX - g.x;
          if (!g.moved && Math.abs(delta) < 5) return;
          g.moved = true; suppressClick.current = true; setDragging(true);
          e.currentTarget.setPointerCapture?.(e.pointerId);
          const shift = delta / Math.max(1, width - 44) * (g.window[1] - g.window[0]);
          move([g.window[0] - shift, g.window[1] - shift], false);
        }}
        onPointerUp={e => { if (gesture.current?.moved) { commit(targetRef.current); e.currentTarget.releasePointerCapture?.(e.pointerId); } gesture.current = undefined; setDragging(false); }}
        onPointerCancel={() => { gesture.current = undefined; setDragging(false); commit(targetRef.current); }}
        onClickCapture={e => { if (suppressClick.current) { e.preventDefault(); e.stopPropagation(); suppressClick.current = false; } }}>
        {Array.from({ length: 6 }, (_, i) => <div className="v3-year-guide" key={i} style={{ left: `${3 + i * 18.8}%` }}><span>{Math.round(camera[0] + span * i / 5)}</span></div>)}
        {dots.map(({ event, row, year: date }, index) => {
          const end = year(event.time?.end) ?? date;
          if (date > camera[1] || end < camera[0]) return null;
          const x = 22 + (Math.max(date, camera[0]) - camera[0]) / span * (width - 44);
          const depth = Math.max(0.45, 1 - (Math.abs(x - width / 2) / (width / 2)) * 0.35);
          const side = row % 2 ? 1 : -1;
          const y = height * 0.5 + side * Math.ceil(row / 2) * 14;
          return (
            <button
              key={event.id}
              className={`v3-dot ${event.time?.kind === "approx" ? "v3-dot--approx" : ""}`}
            style={{ left: x, top: y, "--dot-depth": depth, "--dot-color": LEGEND_COLORS[legendKeyForEventType(event.event_type)], "--dot-delay": `${index % 17 * -230}ms` } as CSSProperties}
            aria-label={`${eventDate(event)} · ${event.title}`} title={`${eventDate(event)} · ${event.title}`}
              onMouseEnter={() => !dragging && setHovered(event)}
              onFocus={() => setHovered(event)}
              onClick={() => onSelect(event.id)}
            >
              <span />
            </button>
          );
        })}
          </div>
        </div>
      </div>
      <footer className="v3-histo-footer">
        <div className="v3-dot-preview v3-histo-preview" aria-live="polite">
          {preview ? (
            <>
              <span>{eventDate(preview)}</span>
              <button type="button" onClick={() => onSelect(preview.id)}>
                {preview.title} ↗
              </button>
            </>
          ) : (
            <span>
              {dots.length} moments in view · drag the field, scroll to zoom, use the timeline below
            </span>
          )}
        </div>
        <div className="v3-histo-phases" role="group" aria-label="Life phases">
          {phases.map((phase) => (
            <button
              key={phase.id}
              type="button"
              onClick={() => navigate(phase.window as WindowYears)}
            >
              {phase.label}
            </button>
          ))}
        </div>
        <HistographyBrush
          bounds={fullBounds}
          window={target}
          histogram={histogram}
          onChange={(next) => move(next, false)}
          onCommit={() => commit(targetRef.current)}
        />
        <div className="v3-histo-extents">
          <span>{fullBounds[0]}</span>
          <span>{fullBounds[1]}</span>
        </div>
      </footer>
      {!dots.length && <p className="v3-histo-empty">No dated events in this selection.</p>}
      {!!undated.length && (
        <div className="v3-undated v3-histo-undated">
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
    </section>
  );
}
