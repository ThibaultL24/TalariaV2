import { useEffect, useRef } from "react";
import type { TimelineEvent } from "@/lib/api";
import { useThemeStore } from "@/stores/theme-store";
const LANES = ["Life", "Works", "Public life", "Places"];
function lane(type: string): number {
  if (["publication", "work"].includes(type)) return 1;
  if (["office", "diplomatic", "treaty", "battle", "siege"].includes(type))
    return 2;
  if (["arrival", "departure", "travel", "residence", "exile"].includes(type))
    return 3;
  return 0;
}
function year(value?: string | null): number | null {
  const match = value?.match(/^(-?\d{4})/);
  return match ? Number(match[1]) : null;
}
export function TimelineCanvas({
  events,
  from,
  to,
  onSelect,
  onZoom,
}: {
  events: TimelineEvent[];
  from: number;
  to: number;
  onSelect: (id: string) => void;
  onZoom: (from: number, to: number) => void;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const hits = useRef<Array<{ x: number; y: number; id: string }>>([]);
  const theme = useThemeStore((s) => s.theme);
  useEffect(() => {
    const node = canvas.current;
    if (!node) return;
    const draw = () => {
      const width = node.clientWidth,
        height = 300,
        ratio = window.devicePixelRatio || 1;
      node.width = width * ratio;
      node.height = height * ratio;
      const ctx = node.getContext("2d");
      if (!ctx) return;
      ctx.scale(ratio, ratio);
      ctx.clearRect(0, 0, width, height);
      const span = Math.max(1, to - from),
        x = (y: number) => 110 + ((y - from) / span) * (width - 135);
      ctx.fillStyle = theme === "dark" ? "#bbb6ac" : "#6D685F";
      ctx.font = "12px sans-serif";
      for (let i = 0; i <= 5; i++) {
        const y = from + (span * i) / 5;
        ctx.fillText(String(Math.round(y)), x(y) - 15, 24);
      }
      hits.current = [];
      LANES.forEach((label, index) => {
        const y = 65 + index * 60;
        ctx.fillText(label, 4, y + 4);
        ctx.strokeStyle = theme === "dark" ? "#444" : "#d8d2c7";
        ctx.beginPath();
        ctx.moveTo(110, y);
        ctx.lineTo(width - 20, y);
        ctx.stroke();
      });
      for (const event of events) {
        if (event.time?.kind === "unknown") continue;
        const start = year(event.time?.start);
        if (start === null || start > to) continue;
        const end = year(event.time?.end) ?? start;
        if (end < from) continue;
        const y = 65 + lane(event.event_type) * 60,
          left = x(Math.max(start, from)),
          right = x(Math.min(end, to));
        ctx.strokeStyle = "#A97645";
        ctx.lineWidth = 3;
        ctx.beginPath();
        ctx.moveTo(left, y);
        ctx.lineTo(right, y);
        ctx.stroke();
        ctx.fillStyle = event.time?.kind === "approx" ? "#A97645" : "#4f6fb0";
        ctx.beginPath();
        ctx.arc(left, y, 5, 0, Math.PI * 2);
        ctx.fill();
        hits.current.push({ x: left, y, id: event.id });
      }
    };
    const observer = new ResizeObserver(draw);
    observer.observe(node);
    draw();
    return () => observer.disconnect();
  }, [events, from, to, theme]);
  const zoom = (factor: number) => {
    const mid = (from + to) / 2,
      half = Math.max(1, ((to - from) * factor) / 2);
    onZoom(Math.floor(mid - half), Math.ceil(mid + half));
  };
  return (
    <section aria-label="Timeline overview">
      <div className="v3-filters">
        <button onClick={() => zoom(0.5)}>Zoom in</button>
        <button onClick={() => zoom(2)}>Zoom out</button>
        <span>
          {from} – {to}
        </span>
      </div>
      <canvas
        ref={canvas}
        style={{ width: "100%", height: 300 }}
        aria-label="Events by life, works, public life and places; choose an event in the accessible list below"
        onClick={(e) => {
          const rect = e.currentTarget.getBoundingClientRect(),
            x = e.clientX - rect.left,
            y = e.clientY - rect.top;
          const hit = hits.current.find(
            (p) => Math.hypot(p.x - x, p.y - y) < 12,
          );
          if (hit) onSelect(hit.id);
        }}
      />
      <p className="v3-note">
        Each dot represents a sourced event. Lines show date ranges; bronze dots
        mark approximate dates. Events without a date remain in the list.
      </p>
    </section>
  );
}
