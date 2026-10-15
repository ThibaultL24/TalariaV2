// web/src/components/timeline/historical-timeline.tsx
import { Application, Container, Graphics } from "pixi.js";
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
import { useThemeStore } from "@/stores/theme-store";
import { HistographyBrush } from "@/features/entity/timeline-histography-brush";
import {
  clamp,
  domainToYearWindow,
  eventColor,
  eventImportance,
  eventYear,
  filterEventsByLod,
  lifePhasePresets,
  randomLaneFromId,
  type TimeDomain,
  yearToX,
  yearWindowToDomain,
  xToYear,
} from "./timeline-utils";
import { aegeanPixiColors } from "@/lib/aegean-palette";
import "./historical-timeline.css";

type RenderedPoint = {
  event: TimelineEvent;
  year: number;
  x: number;
  y: number;
  targetX: number;
  targetY: number;
  baseY: number;
  radius: number;
  color: number;
};

export interface HistoricalTimelineProps {
  events: TimelineEvent[];
  bounds: [number, number];
  from: number;
  to: number;
  spotlightEventId?: string;
  zoomToEventId?: string;
  zoomToEventToken?: number;
  onSelect: (id: string) => void;
  onZoom: (from: number, to: number) => void;
  mode?: "desktop" | "fullscreen";
}

export function HistoricalTimeline({
  events,
  bounds,
  from,
  to,
  spotlightEventId,
  zoomToEventId,
  zoomToEventToken = 0,
  onSelect,
  onZoom,
  mode = "desktop",
}: HistoricalTimelineProps) {
  const theme = useThemeStore((s) => s.theme);
  const themeRef = useRef(theme);
  themeRef.current = theme;
  const pixiAppRef = useRef<Application | null>(null);
  const canvasHostRef = useRef<HTMLDivElement>(null);
  const absoluteDomain = useMemo(() => yearWindowToDomain(bounds), [bounds[0], bounds[1]]);
  const [domain, setDomain] = useState<TimeDomain>(() => yearWindowToDomain([from, to]));
  const [hovered, setHovered] = useState<TimelineEvent | null>(null);
  const [hoverPosition, setHoverPosition] = useState({ x: 0, y: 0 });
  const [activeLegend, setActiveLegend] = useState<Set<LegendKey> | null>(null);
  const { locale, t } = useI18n();
  const [categoriesOpen, setCategoriesOpen] = useState(false);
  const domainRef = useRef(domain);
  const recalcSceneRef = useRef<() => void>(() => {});
  const pixiReadyRef = useRef(false);
  const spotlightRef = useRef<string | undefined>(spotlightEventId);
  spotlightRef.current = spotlightEventId;
  domainRef.current = domain;

  useEffect(() => {
    if (!zoomToEventId || zoomToEventToken === 0) return;
    const event = events.find((item) => item.id === zoomToEventId);
    if (!event) return;
    const year = eventYear(event);
    if (year === null) return;
    const fullSpan = Math.max(1, absoluteDomain.end - absoluteDomain.start);
    const span = Math.min(fullSpan, Math.max(10, fullSpan * 0.07));
    const start = clamp(year - span / 2, absoluteDomain.start, absoluteDomain.end - span);
    applyDomain({ start, end: start + span }, true);
    setHovered(event);
    recalcSceneRef.current();
  }, [zoomToEventId, zoomToEventToken, events, absoluteDomain.end, absoluteDomain.start]);

  useEffect(() => {
    recalcSceneRef.current();
  }, [spotlightEventId]);

  useEffect(() => {
    setDomain(yearWindowToDomain([from, to]));
  }, [from, to]);

  const legendCounts = useMemo(() => {
    const counts = new Map<LegendKey, number>();
    for (const event of events) {
      const key = legendKeyForEventType(event.event_type);
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return counts;
  }, [events]);

  const datedEvents = useMemo(
    () =>
      events.filter((event) => {
        if (eventYear(event) === null) return false;
        if (!activeLegend?.size) return true;
        return activeLegend.has(legendKeyForEventType(event.event_type));
      }),
    [events, activeLegend],
  );

  const phases = useMemo(() => lifePhasePresets(bounds), [bounds[0], bounds[1]]);

  function applyDomain(next: TimeDomain, commit: boolean) {
    const clamped: TimeDomain = {
      start: clamp(next.start, absoluteDomain.start, absoluteDomain.end - 1),
      end: clamp(next.end, absoluteDomain.start + 1, absoluteDomain.end),
    };
    if (clamped.end <= clamped.start) {
      clamped.end = clamped.start + 1;
    }
    setDomain(clamped);
    if (commit) {
      const [a, b] = domainToYearWindow(clamped);
      onZoom(Math.floor(a), Math.ceil(b));
    }
  }

  function toggleLegend(key: LegendKey) {
    setActiveLegend((current) => {
      if (!current) return new Set([key]);
      const next = new Set(current);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next.size ? next : null;
    });
  }

  useEffect(() => {
    const host = canvasHostRef.current;
    if (!host) return;

    let destroyed = false;
    let ready = false;
    let tornDown = false;
    let canvasEl: HTMLCanvasElement | undefined;
    const app = new Application();
    let points: RenderedPoint[] = [];
    let buckets: Map<number, RenderedPoint[]> = new Map();
    let dragging = false;
    let dragStartX = 0;
    let dragDomain: TimeDomain | null = null;
    let pointerX = -1000;
    let pointerY = -1000;
    let currentHovered: RenderedPoint | undefined;

    const dots = new Graphics();
    const cursor = new Graphics();
    const highlight = new Graphics();
    const stage = new Container();

    const rebuildBuckets = () => {
      buckets = new Map();
      for (const point of points) {
        const key = Math.floor(point.x / 24);
        const list = buckets.get(key) ?? [];
        list.push(point);
        buckets.set(key, list);
      }
    };

    const calculatePoints = () => {
      if (!ready || destroyed) return;
      const width = app.screen.width;
      const height = app.screen.height;
      const top = 72;
      const bottom = 88;
      const usable = Math.max(120, height - top - bottom);
      const visible = filterEventsByLod(datedEvents, domainRef.current);

      points = visible
        .map((event) => {
          const year = eventYear(event)!;
          const x = yearToX(year, domainRef.current, width);
          const lane = randomLaneFromId(event.id);
          const baseY = top + lane * usable;
          const importance = eventImportance(event);
          const radius = 1.8 + importance * 3.2;
          return {
            event,
            year,
            x,
            y: baseY,
            targetX: x,
            targetY: baseY,
            baseY,
            radius,
            color: eventColor(event),
          };
        })
        .filter(
          (point) =>
            point.year >= domainRef.current.start && point.year <= domainRef.current.end,
        );
      rebuildBuckets();
    };

    const updateTargets = () => {
      if (!ready || destroyed) return;
      const width = app.screen.width;
      for (const point of points) {
        point.targetX = yearToX(point.year, domainRef.current, width);
      }
    };

    const findNearest = (x: number, y: number) => {
      const bucket = Math.floor(x / 24);
      const candidates = [
        ...(buckets.get(bucket - 1) ?? []),
        ...(buckets.get(bucket) ?? []),
        ...(buckets.get(bucket + 1) ?? []),
      ];
      let nearest: RenderedPoint | undefined;
      let nearestDistance = 20;
      for (const point of candidates) {
        const dx = point.x - x;
        const dy = point.y - y;
        const distance = Math.hypot(dx, dy);
        if (distance < nearestDistance) {
          nearest = point;
          nearestDistance = distance;
        }
      }
      return nearest;
    };

    const draw = () => {
      if (!ready || destroyed) return;
      dots.clear();
      const spotlightId = spotlightRef.current;
      for (const point of points) {
        if (point.x < -24 || point.x > app.screen.width + 24) continue;
        const alpha = point.event.map_eligible ? 0.95 : 0.62;
        dots.circle(point.x, point.y, point.radius).fill({ color: point.color, alpha });
        if (spotlightId && point.event.id === spotlightId) {
          const palette = aegeanPixiColors(themeRef.current === "dark" ? "dark" : "light");
          const ring = palette.selectedRing;
          dots
            .circle(point.x, point.y, point.radius + 7)
            .stroke({ width: 2, color: ring, alpha: 0.92 });
          dots
            .circle(point.x, point.y, point.radius + 12)
            .stroke({ width: 1, color: ring, alpha: 0.38 });
        }
      }
      drawCursor();
    };

    const drawCursor = () => {
      cursor.clear();
      highlight.clear();
      if (!currentHovered) return;
      const palette = aegeanPixiColors(themeRef.current === "dark" ? "dark" : "light");
      const guide = palette.primary;
      cursor
        .moveTo(currentHovered.x, 0)
        .lineTo(currentHovered.x, app.screen.height)
        .stroke({ width: 1, color: guide, alpha: 0.18 });
      highlight
        .circle(currentHovered.x, currentHovered.y, currentHovered.radius + 5)
        .stroke({ width: 1, color: guide, alpha: 0.85 });
      highlight
        .circle(currentHovered.x, currentHovered.y, currentHovered.radius + 1.5)
        .fill({ color: guide, alpha: 1 });
    };

    const onPointerMove = (event: { global: { x: number; y: number } }) => {
      if (!ready || destroyed) return;
      pointerX = event.global.x;
      pointerY = event.global.y;

      if (dragging && dragDomain) {
        const width = app.screen.width;
        const dx = event.global.x - dragStartX;
        const duration = dragDomain.end - dragDomain.start;
        const yearDelta = -(dx / width) * duration;
        domainRef.current = {
          start: dragDomain.start + yearDelta,
          end: dragDomain.end + yearDelta,
        };
        updateTargets();
        return;
      }

      currentHovered = findNearest(pointerX, pointerY);
      if (currentHovered) {
        setHovered(currentHovered.event);
        setHoverPosition({ x: currentHovered.x, y: currentHovered.y });
        app.canvas.style.cursor = "pointer";
      } else {
        setHovered(null);
        app.canvas.style.cursor = "grab";
      }
      drawCursor();
    };

    const onPointerDown = (event: { global: { x: number; y: number } }) => {
      if (!ready || destroyed) return;
      const nearest = findNearest(event.global.x, event.global.y);
      if (nearest) {
        onSelect(nearest.event.id);
        return;
      }
      dragging = true;
      dragStartX = event.global.x;
      dragDomain = { ...domainRef.current };
      app.canvas.style.cursor = "grabbing";
    };

    const onPointerUp = () => {
      if (!ready || destroyed) return;
      if (dragging) {
        applyDomain(domainRef.current, true);
      }
      dragging = false;
      dragDomain = null;
      app.canvas.style.cursor = "grab";
    };

    const onWheel = (event: WheelEvent) => {
      if (!ready || destroyed) return;
      event.preventDefault();
      const rect = app.canvas.getBoundingClientRect();
      const localX = event.clientX - rect.left;
      const width = app.screen.width;
      const current = domainRef.current;
      const duration = current.end - current.start;
      const anchorYear = xToYear(localX, current, width);
      const factor = event.deltaY > 0 ? 1.18 : 0.84;
      let nextDuration = duration * factor;
      const minDuration = 1;
      const maxDuration = absoluteDomain.end - absoluteDomain.start;
      nextDuration = clamp(nextDuration, minDuration, maxDuration);
      const ratio = localX / width;
      const nextStart = anchorYear - nextDuration * ratio;
      const nextEnd = nextStart + nextDuration;
      domainRef.current = {
        start: clamp(nextStart, absoluteDomain.start, absoluteDomain.end - minDuration),
        end: clamp(nextEnd, absoluteDomain.start + minDuration, absoluteDomain.end),
      };
      calculatePoints();
      applyDomain(domainRef.current, true);
    };

    const tick = () => {
      if (!ready || destroyed) return;
      let moving = false;
      for (const point of points) {
        const dx = pointerX - point.x;
        const dy = pointerY - point.y;
        const distance = Math.hypot(dx, dy);
        let repulseY = 0;
        if (distance > 0 && distance < 100) {
          const force = (1 - distance / 100) * 10;
          repulseY = (-dy / distance) * force;
        }
        const targetY = point.baseY + repulseY;
        point.targetY = targetY;

        const ddx = point.targetX - point.x;
        const ddy = point.targetY - point.y;
        if (Math.abs(ddx) > 0.08) {
          point.x += ddx * 0.14;
          moving = true;
        } else {
          point.x = point.targetX;
        }
        if (Math.abs(ddy) > 0.08) {
          point.y += ddy * 0.18;
          moving = true;
        } else {
          point.y = point.targetY;
        }
      }
      if (moving) draw();
    };

    const teardown = () => {
      if (tornDown) return;
      tornDown = true;
      ready = false;
      pixiAppRef.current = null;
      app.ticker?.remove(tick);
      canvasEl?.removeEventListener("wheel", onWheel);
      canvasEl = undefined;
      try {
        app.destroy(true, { children: true });
      } catch {
        // Pixi may already be torn down (React StrictMode double mount).
      }
    };

    const pixiBackground = () =>
      aegeanPixiColors(themeRef.current === "dark" ? "dark" : "light").background;

    const init = async () => {
      await app.init({
        resizeTo: host,
        background: pixiBackground(),
        antialias: true,
        autoDensity: true,
        resolution: Math.min(window.devicePixelRatio, 2),
        powerPreference: "high-performance",
      });
      if (destroyed) {
        teardown();
        return;
      }
      canvasEl = app.canvas;
      pixiAppRef.current = app;
      host.appendChild(canvasEl);
      stage.addChild(dots);
      stage.addChild(cursor);
      stage.addChild(highlight);
      app.stage.addChild(stage);
      app.stage.eventMode = "static";
      app.stage.hitArea = app.screen;
      ready = true;
      pixiReadyRef.current = true;
      recalcSceneRef.current = () => {
        if (!ready || destroyed) return;
        calculatePoints();
        draw();
      };
      calculatePoints();
      draw();
      app.stage.on("globalpointermove", onPointerMove);
      app.stage.on("pointerdown", onPointerDown);
      app.stage.on("pointerup", onPointerUp);
      app.stage.on("pointerupoutside", onPointerUp);
      canvasEl.addEventListener("wheel", onWheel, { passive: false });
      app.ticker.add(tick);
    };

    const resizeObserver = new ResizeObserver(() => {
      if (!ready || destroyed) return;
      calculatePoints();
      draw();
    });
    resizeObserver.observe(host);
    const initPromise = init().catch(() => {
      // init aborted or failed during StrictMode remount
    });

    return () => {
      destroyed = true;
      pixiReadyRef.current = false;
      recalcSceneRef.current = () => {};
      resizeObserver.disconnect();
      void initPromise.then(teardown);
    };
  }, [datedEvents, absoluteDomain.start, absoluteDomain.end, onSelect]);

  useEffect(() => {
    domainRef.current = domain;
    if (pixiReadyRef.current) {
      recalcSceneRef.current();
    }
  }, [domain.start, domain.end]);

  useEffect(() => {
    const app = pixiAppRef.current;
    if (!app?.renderer) return;
    app.renderer.background.color = aegeanPixiColors(theme === "dark" ? "dark" : "light").background;
    recalcSceneRef.current();
  }, [theme]);

  const histogram = useMemo(() => {
    const bins = Array<number>(64).fill(0);
    for (const event of datedEvents) {
      const year = eventYear(event);
      if (year === null) continue;
      const t =
        (year - absoluteDomain.start) / Math.max(1, absoluteDomain.end - absoluteDomain.start);
      bins[Math.min(63, Math.max(0, Math.floor(t * 64)))]++;
    }
    return bins;
  }, [datedEvents, absoluteDomain]);

  const yearWindow = domainToYearWindow(domain);

  return (
    <section
      className={`historical-timeline${mode === "fullscreen" ? " historical-timeline--fullscreen" : ""}`}
      aria-label="Interactive timeline"
      data-mode={mode}
    >
      {mode === "fullscreen" ? (
        <button
          type="button"
          className="historical-timeline__drawer-toggle"
          aria-expanded={categoriesOpen}
          onClick={() => setCategoriesOpen((open) => !open)}
        >
          {t.immersiveCategories}
        </button>
      ) : null}
      <div className="historical-timeline__body">
        {mode === "fullscreen" && categoriesOpen ? (
          <button
            type="button"
            className="historical-timeline__drawer-scrim"
            aria-label={t.close}
            onClick={() => setCategoriesOpen(false)}
          />
        ) : null}
        <aside
          className={`historical-timeline__categories${mode === "fullscreen" && categoriesOpen ? " is-open" : ""}`}
          aria-label="Event categories"
          hidden={mode === "fullscreen" && !categoriesOpen}
        >
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
                <span
                  className="historical-timeline__swatch"
                  style={{ background: LEGEND_COLORS[key] } as CSSProperties}
                />
                {legendLabel(key, locale)} <span>{count}</span>
              </button>
            ))}
        </aside>
        <div className="historical-timeline__canvas-host" ref={canvasHostRef}>
          <div className="historical-timeline__header">
            <span>{Math.round(domain.start)}</span>
            <span className="historical-timeline__header-label">LIFE IN TIME</span>
            <span>{Math.round(domain.end)}</span>
          </div>
          {hovered && (
            <div
              className="historical-timeline__tooltip"
              style={{ left: hoverPosition.x, top: hoverPosition.y }}
            >
              <div className="historical-timeline__tooltip-year">{eventDate(hovered)}</div>
              <div className="historical-timeline__tooltip-title">{hovered.title}</div>
            </div>
          )}
        </div>
      </div>
      <footer className="historical-timeline__footer">
        <div className="historical-timeline__phases" role="group" aria-label="Life phases">
          {phases.map((phase) => (
            <button
              key={phase.id}
              type="button"
              onClick={() => applyDomain(yearWindowToDomain(phase.window), true)}
            >
              {phase.label}
            </button>
          ))}
        </div>
        <HistographyBrush
          bounds={bounds}
          window={yearWindow}
          histogram={histogram}
          onChange={(next) => applyDomain(yearWindowToDomain(next), false)}
          onCommit={() => applyDomain(domainRef.current, true)}
        />
      </footer>
    </section>
  );
}
