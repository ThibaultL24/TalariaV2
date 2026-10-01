import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { TimelineEvent } from "@/lib/api";
import { eventDate } from "@/lib/entity-views";
import { LEGEND_COLORS, legendKeyForEventType } from "@/lib/event-legend";

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
  const dots = useMemo(() => layoutDots(events, fullBounds[0], fullBounds[1], width), [events, fullBounds[0], fullBounds[1], width]);
  const height = dots.reduce((max, dot) => Math.max(max, 200 + dot.row * 24), 380);
  const span = Math.max(1, camera[1] - camera[0]);
  const preview = hovered && events.find(event => event.id === hovered.id);
  const undated = events.filter(event => event.time?.kind === "unknown" || year(event.time?.start) === null);
  const histogram = useMemo(() => {
    const bins = Array<number>(64).fill(0);
    for (const dot of dots) bins[Math.min(63, Math.max(0, Math.floor((dot.year - fullBounds[0]) / (fullBounds[1] - fullBounds[0]) * 64)))]++;
    return bins;
  }, [dots, fullBounds[0], fullBounds[1]]);
  const navigate = (next: WindowYears) => { move(next, false); commit(clampWindow(next, fullBounds)); };
  return <section className="v3-constellation v3-time-space" aria-label="Interactive timeline">
    <div className="v3-timeline-toolbar">
      <div><span className="v3-eyebrow">A LIFE IN MOMENTS</span><h2>{Math.round(target[0])} — {Math.round(target[1])}</h2></div>
      <div className="v3-timeline-controls">
        <button onClick={() => navigate([target[0] - span / 4, target[1] - span / 4])} aria-label="Earlier period">←</button>
        <button onClick={() => navigate(zoomWindow(target, 2, .5, fullBounds))} aria-label="Zoom out">−</button>
        <button onClick={() => navigate(zoomWindow(target, .5, .5, fullBounds))} aria-label="Zoom in">+</button>
        <button onClick={() => navigate([target[0] + span / 4, target[1] + span / 4])} aria-label="Later period">→</button>
        <button onClick={() => navigate(fullBounds)} aria-label="Reset period">↺</button>
      </div>
    </div>
    <p className="v3-note">Drag to travel through time · Scroll to zoom · Click a point to read its story</p>
    <div className="v3-dot-scroll">
      <div ref={host} className={`v3-dot-field ${dragging ? "is-dragging" : ""}`} style={{ height }}
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
          const depth = Math.max(.3, 1 - Math.abs(x - width / 2) / (width / 2) * .6);
          const side = row % 2 ? 1 : -1;
          const y = height / 2 + side * Math.ceil(row / 2) * 24 + Math.sin(x / width * Math.PI * 2) * 45;
          return <button key={event.id} className={`v3-dot ${event.time?.kind === "approx" ? "v3-dot--approx" : ""}`}
            style={{ left: x, top: y, "--dot-depth": depth, "--dot-color": LEGEND_COLORS[legendKeyForEventType(event.event_type)], "--dot-delay": `${index % 17 * -230}ms` } as CSSProperties}
            aria-label={`${eventDate(event)} · ${event.title}`} title={`${eventDate(event)} · ${event.title}`}
            onMouseEnter={() => !dragging && setHovered(event)} onFocus={() => setHovered(event)} onClick={() => onSelect(event.id)}><span /></button>;
        })}
      </div>
    </div>
    <div className="v3-dot-preview" aria-live="polite">{preview ? <><span>{eventDate(preview)}</span><button onClick={() => onSelect(preview.id)}>{preview.title} ↗</button></> : <span>{dots.length} sourced moments · Tab and Enter also open each point.</span>}</div>
    <div className="v3-period-navigator">
      <div className="v3-density" aria-hidden="true">{histogram.map((count, index) => <span key={index} style={{ height: `${Math.max(3, count / Math.max(1, ...histogram) * 100)}%` }} />)}
        <div className="v3-period-window" style={{ left: `${(target[0] - fullBounds[0]) / (fullBounds[1] - fullBounds[0]) * 100}%`, width: `${(target[1] - target[0]) / (fullBounds[1] - fullBounds[0]) * 100}%` }} />
      </div>
      <div className="v3-period-handles">
        <label>Period start <input type="range" min={fullBounds[0]} max={fullBounds[1] - 1} value={target[0]} onChange={e => move([Math.min(Number(e.target.value), target[1] - 1), target[1]])} /></label>
        <label>Period end <input type="range" min={fullBounds[0] + 1} max={fullBounds[1]} value={target[1]} onChange={e => move([target[0], Math.max(Number(e.target.value), target[0] + 1)])} /></label>
      </div>
      <div className="v3-period-extents"><span>{fullBounds[0]}</span><span>Explore the whole life</span><span>{fullBounds[1]}</span></div>
    </div>
    {!dots.length && <p>No dated events available.</p>}
    {!!undated.length && <div className="v3-undated"><span>Date not established</span>{undated.map(event => <button key={event.id} onClick={() => onSelect(event.id)} aria-label={event.title} title={event.title}>●</button>)}</div>}
    <p className="v3-note">One point per sourced event. Hollow points indicate approximate dates. Full date ranges and evidence appear in the event details.</p>
  </section>;
}
