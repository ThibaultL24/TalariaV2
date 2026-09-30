import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { TimelineEvent } from "@/lib/api";
import { eventDate } from "@/lib/entity-views";
import { LEGEND_COLORS, legendKeyForEventType } from "@/lib/event-legend";

function year(value?: string | null): number | null {
  const match = value?.match(/^(-?\d{4,})(?:-|$)/);
  return match ? Number(match[1]) : null;
}

// Keep the horizontal date exact; stack neighbours vertically so every target is reachable.
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
    return [{ event, x, row }];
  });
}

export function TimelineCanvas({ events, from, to, onSelect, onZoom }: {
  events: TimelineEvent[]; from: number; to: number;
  onSelect: (id: string) => void; onZoom: (from: number, to: number) => void;
}) {
  const host = useRef<HTMLDivElement>(null);
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
  const dots = useMemo(() => layoutDots(events, from, to, width), [events, from, to, width]);
  const height = dots.reduce((height, dot) => Math.max(height, 80 + dot.row * 24), 320);
  const preview = hovered && events.find(event => event.id === hovered.id);
  const undated = events.filter((event) => event.time?.kind === "unknown" || year(event.time?.start) === null);
  const zoom = (factor: number) => {
    const mid = (from + to) / 2, half = Math.max(1, (to - from) * factor / 2);
    onZoom(Math.floor(mid - half), Math.ceil(mid + half));
  };
  const shift = (direction: number) => {
    const step = Math.max(1, Math.round((to - from) / 4)) * direction;
    onZoom(from + step, to + step);
  };
  return <section className="v3-constellation" aria-label="Interactive timeline">
    <div className="v3-timeline-toolbar">
      <div><span className="v3-eyebrow">A LIFE IN MOMENTS</span><h2>{from} — {to}</h2></div>
      <div className="v3-timeline-controls">
        <button onClick={() => shift(-1)} aria-label="Earlier period">←</button>
        <button onClick={() => zoom(2)} aria-label="Zoom out">−</button>
        <button onClick={() => zoom(.5)} aria-label="Zoom in">+</button>
        <button onClick={() => shift(1)} aria-label="Later period">→</button>
      </div>
    </div>
    <p className="v3-note">{dots.length} moments · Hover to discover, click to read the evidence. Use Tab and Enter to explore by keyboard.</p>
    <div className="v3-dot-scroll">
      <div ref={host} className="v3-dot-field" style={{ height }}>
        {Array.from({ length: 6 }, (_, i) => <div className="v3-year-guide" key={i} style={{ left: `${3 + i * 18.8}%` }}>
          <span>{Math.round(from + (to - from) * i / 5)}</span>
        </div>)}
        {dots.map(({ event, x, row }, index) => <button key={event.id}
          className={`v3-dot ${event.time?.kind === "approx" ? "v3-dot--approx" : ""}`}
          style={{ left: x, top: height - 48 - row * 24, "--dot-color": LEGEND_COLORS[legendKeyForEventType(event.event_type)], "--dot-delay": `${Math.min(index * 8, 650)}ms` } as CSSProperties}
          aria-label={`${eventDate(event)} · ${event.title}`} title={`${eventDate(event)} · ${event.title}`}
          onMouseEnter={() => setHovered(event)} onFocus={() => setHovered(event)}
          onClick={() => onSelect(event.id)}><span /></button>)}
      </div>
    </div>
    <div className="v3-dot-preview" aria-live="polite">
      {preview ? <><span>{eventDate(preview)}</span><button onClick={() => onSelect(preview.id)}>{preview.title} ↗</button></> : <span>Select a point to explore its story.</span>}
    </div>
    {!dots.length && <p>No dated events in this period.</p>}
    {!!undated.length && <div className="v3-undated"><span>Date not established</span>{undated.map((event) => <button key={event.id} onClick={() => onSelect(event.id)} aria-label={event.title} title={event.title}>●</button>)}</div>}
    <p className="v3-note">One point per sourced occurrence. Hollow points indicate approximate dates; full date ranges and source precision appear in the event details.</p>
  </section>;
}
