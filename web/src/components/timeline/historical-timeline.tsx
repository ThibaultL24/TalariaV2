// web/src/components/timeline/historical-timeline.tsx
import { Application, Container, Graphics } from "pixi.js";
import { useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";
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
  axisTicks,
  clamp,
  domainToYearWindow,
  eventYear,
  filterEventsByLod,
  formatYearLabel,
  layoutConstellation,
  lifePhasePresets,
  type DocumentedLifePhase,
  type TimeDomain,
  xToYear,
  yearWindowToDomain,
} from "./timeline-utils";
import { aegeanPixiColors } from "@/lib/aegean-palette";
import "./historical-timeline.css";

function pixiThemeFromDom(fallback: "light" | "dark"): "light" | "dark" {
  const attr = document.documentElement.dataset.theme;
  if (attr === "dark" || attr === "light") return attr;
  return fallback;
}

type Particle = {
  id: string;
  event: TimelineEvent;
  year: number;
  x: number;
  y: number;
  targetX: number;
  targetY: number;
  baseY: number;
  radius: number;
  color: number;
  alpha: number;
  targetAlpha: number;
  rank: "major" | "secondary" | "minor";
  importance: number;
  shownAlpha: number;
};

export interface HistoricalTimelineProps {
  events: TimelineEvent[];
  bounds: [number, number];
  from: number;
  to: number;
  spotlightEventId?: string;
  zoomToEventId?: string;
  zoomToEventToken?: number;
  lifePhases?: DocumentedLifePhase[];
  onSelect: (id: string) => void;
  onZoom: (from: number, to: number) => void;
  mode?: "desktop" | "fullscreen";
  coverage?: { timeline: number; mapped: number };
}

export function HistoricalTimeline({
  events,
  bounds,
  from,
  to,
  spotlightEventId,
  zoomToEventId,
  zoomToEventToken = 0,
  lifePhases,
  onSelect,
  onZoom,
  mode = "desktop",
  coverage,
}: HistoricalTimelineProps) {
  const theme = useThemeStore((s) => s.theme);
  const themeRef = useRef(theme);
  themeRef.current = theme;
  const pixiAppRef = useRef<Application | null>(null);
  const canvasHostRef = useRef<HTMLDivElement>(null);
  const absoluteDomain = useMemo(() => yearWindowToDomain(bounds), [bounds[0], bounds[1]]);
  const absoluteRef = useRef(absoluteDomain);
  absoluteRef.current = absoluteDomain;
  const [domain, setDomain] = useState<TimeDomain>(() => yearWindowToDomain([from, to]));
  const [hovered, setHovered] = useState<TimelineEvent | null>(null);
  const [hoverPosition, setHoverPosition] = useState({ x: 0, y: 0 });
  const [activeLegend, setActiveLegend] = useState<Set<LegendKey> | null>(null);
  const { locale, t } = useI18n();
  const [categoriesOpen, setCategoriesOpen] = useState(false);
  const domainRef = useRef(domain);
  const syncSceneRef = useRef<() => void>(() => {});
  const drawSceneRef = useRef<() => void>(() => {});
  const pixiReadyRef = useRef(false);
  const spotlightRef = useRef<string | undefined>(spotlightEventId);
  spotlightRef.current = spotlightEventId;
  domainRef.current = domain;
  const onSelectRef = useRef(onSelect);
  onSelectRef.current = onSelect;
  const onZoomRef = useRef(onZoom);
  onZoomRef.current = onZoom;
  const publishFrame = useRef(0);
  const commitTimer = useRef(0);

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
  const datedRef = useRef(datedEvents);
  datedRef.current = datedEvents;

  const phases = useMemo(
    () => lifePhasePresets(bounds, lifePhases),
    [bounds[0], bounds[1], lifePhases],
  );

  function publishDomain() {
    cancelAnimationFrame(publishFrame.current);
    publishFrame.current = requestAnimationFrame(() => {
      const next = domainRef.current;
      setDomain({ start: next.start, end: next.end });
    });
  }

  function scheduleZoomCommit() {
    window.clearTimeout(commitTimer.current);
    commitTimer.current = window.setTimeout(() => {
      const [start, end] = domainToYearWindow(domainRef.current);
      onZoomRef.current(start, end);
    }, 180);
  }

  function applyDomain(next: TimeDomain, commit: boolean) {
    const limits = absoluteRef.current;
    const clamped: TimeDomain = {
      start: clamp(next.start, limits.start, limits.end - 1),
      end: clamp(next.end, limits.start + 1, limits.end),
    };
    if (clamped.end - clamped.start < 1) {
      clamped.end = Math.min(limits.end, clamped.start + 1);
      clamped.start = Math.max(limits.start, clamped.end - 1);
    }
    domainRef.current = clamped;
    setDomain(clamped);
    syncSceneRef.current();
    if (commit) {
      window.clearTimeout(commitTimer.current);
      const [start, end] = domainToYearWindow(clamped);
      onZoomRef.current(start, end);
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

  function phaseLabel(id: string, label: string, documented: boolean): string {
    if (documented) return label;
    if (id === "full") return t.timelinePhaseFull;
    if (id === "opening") return t.timelinePhaseOpening;
    if (id === "middle") return t.timelinePhaseMiddle;
    if (id === "closing") return t.timelinePhaseClosing;
    return label;
  }

  useEffect(() => {
    if (!zoomToEventId || zoomToEventToken === 0) return;
    const event = events.find((item) => item.id === zoomToEventId);
    if (!event) return;
    const year = eventYear(event);
    if (year === null) return;
    const fullSpan = Math.max(1, absoluteDomain.end - absoluteDomain.start);
    const span = Math.min(fullSpan, Math.max(8, fullSpan * 0.08));
    const start = clamp(year - span / 2, absoluteDomain.start, absoluteDomain.end - span);
    applyDomain({ start, end: start + span }, true);
    setHovered(event);
  }, [zoomToEventId, zoomToEventToken]);

  useEffect(() => {
    setDomain(yearWindowToDomain([from, to]));
    domainRef.current = yearWindowToDomain([from, to]);
    syncSceneRef.current();
  }, [from, to]);

  useEffect(() => {
    drawSceneRef.current();
  }, [spotlightEventId]);

  useEffect(() => {
    syncSceneRef.current();
  }, [datedEvents]);

  useEffect(() => {
    const host = canvasHostRef.current;
    if (!host) return;

    let destroyed = false;
    let ready = false;
    let tornDown = false;
    let canvasEl: HTMLCanvasElement | undefined;
    const app = new Application();
    let particles: Particle[] = [];
    let buckets = new Map<number, Particle[]>();
    let dragging = false;
    let dragMoved = false;
    let dragStartX = 0;
    let dragDomain: TimeDomain | null = null;
    let pointerX = -1000;
    let pointerY = -1000;
    let currentHovered: Particle | undefined;
    let downId: string | null = null;
    const pointers = new Map<number, { x: number; y: number }>();
    let pinchDistance = 0;

    const dots = new Graphics();
    const guides = new Graphics();
    const stage = new Container();

    const pixiTheme = () => pixiThemeFromDom(themeRef.current === "dark" ? "dark" : "light");

    const reduceMotion = () =>
      typeof window.matchMedia === "function"
      && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    const rebuildBuckets = () => {
      buckets = new Map();
      for (const point of particles) {
        if (point.alpha < 0.05) continue;
        const key = Math.floor(point.x / 28);
        const list = buckets.get(key) ?? [];
        list.push(point);
        buckets.set(key, list);
      }
    };

    const findNearest = (x: number, y: number) => {
      const bucket = Math.floor(x / 28);
      const candidates = [
        ...(buckets.get(bucket - 1) ?? []),
        ...(buckets.get(bucket) ?? []),
        ...(buckets.get(bucket + 1) ?? []),
      ];
      let nearest: Particle | undefined;
      let nearestDistance = 18;
      for (const point of candidates) {
        const distance = Math.hypot(point.x - x, point.y - y);
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
      guides.clear();
      const palette = aegeanPixiColors(pixiTheme());
      const width = app.screen.width;
      const height = app.screen.height;
      const spotlightId = spotlightRef.current;
      const hoveredId = currentHovered?.id;
      const ordered = [...particles].sort((a, b) => {
        const rank = { minor: 0, secondary: 1, major: 2 };
        return rank[a.rank] - rank[b.rank];
      });
      for (const point of ordered) {
        if (point.alpha < 0.02 || point.x < -40 || point.x > width + 40) continue;
        const hovered = point.id === hoveredId;
        const selected = point.id === spotlightId;
        let alpha = point.alpha;
        if (hoveredId && !hovered) alpha *= point.rank === "major" ? 0.82 : 0.55;
        const radius = point.radius * (hovered ? 1.35 : 1);
        dots.circle(point.x, point.y, radius).fill({ color: point.color, alpha });
        if (selected || hovered) {
          dots.circle(point.x, point.y, radius + (selected ? 6 : 4)).stroke({
            width: selected ? 1.5 : 1,
            color: selected ? palette.selectedRing : point.color,
            alpha: 0.95,
          });
        }
      }
      if (currentHovered) {
        guides
          .moveTo(currentHovered.x, 36)
          .lineTo(currentHovered.x, height - 28)
          .stroke({ width: 1, color: palette.primary, alpha: 0.28 });
        guides
          .circle(currentHovered.x, currentHovered.y, currentHovered.radius + 6)
          .stroke({ width: 1.25, color: palette.primary, alpha: 0.9 });
      }
      const domainNow = domainRef.current;
      for (const tick of axisTicks(domainNow)) {
        const x = ((tick - domainNow.start) / Math.max(1e-6, domainNow.end - domainNow.start)) * width;
        if (x < 8 || x > width - 8) continue;
        guides
          .moveTo(x, height - 22)
          .lineTo(x, height - 14)
          .stroke({ width: 1, color: palette.guideInk, alpha: 0.35 });
      }
      if (app.renderer) app.render();
    };
    drawSceneRef.current = draw;

    const placeAll = () => {
      if (!ready || destroyed) return;
      const hostEl = canvasHostRef.current;
      if (hostEl) hostEl.dataset.placed = String(Number(hostEl.dataset.placed ?? "0") + 1);
      const width = app.screen.width;
      const height = app.screen.height;
      const placed = layoutConstellation(datedRef.current, domainRef.current, width, height);
      const byId = new Map(placed.map((point) => [point.event.id, point]));
      const seen = new Set<string>();
      for (const particle of particles) {
        const next = byId.get(particle.id);
        if (!next) {
          particle.targetAlpha = 0;
          continue;
        }
        seen.add(particle.id);
        particle.event = next.event;
        particle.year = next.time;
        particle.baseY = next.y;
        particle.targetY = next.y;
        particle.radius = next.radius;
        particle.color = next.color;
        particle.rank = next.rank;
        particle.importance = next.importance;
        particle.shownAlpha = next.alpha;
      }
      for (const point of placed) {
        if (seen.has(point.event.id)) continue;
        particles.push({
          id: point.event.id,
          event: point.event,
          year: point.time,
          x: point.x,
          y: point.y,
          targetX: point.x,
          targetY: point.y,
          baseY: point.y,
          radius: point.radius,
          color: point.color,
          alpha: point.alpha,
          targetAlpha: point.alpha,
          rank: point.rank,
          importance: point.importance,
          shownAlpha: point.alpha,
        });
      }
      applyView();
    };

    const applyView = () => {
      if (!ready || destroyed) return;
      const width = app.screen.width;
      const domainNow = domainRef.current;
      const visible = new Set(filterEventsByLod(datedRef.current, domainNow).map((event) => event.id));
      for (const particle of particles) {
        const year = eventYear(particle.event) ?? particle.year;
        const inside = year >= domainNow.start && year <= domainNow.end;
        particle.targetX = ((particle.year - domainNow.start) / Math.max(1e-6, domainNow.end - domainNow.start)) * width;
        const show = inside && visible.has(particle.id);
        particle.targetAlpha = show ? particle.shownAlpha : 0;
        particle.x = particle.targetX;
        particle.alpha = particle.targetAlpha;
        particle.y = particle.baseY;
      }
      rebuildBuckets();
      draw();
    };
    syncSceneRef.current = placeAll;

    const localPoint = (event: PointerEvent) => {
      const rect = canvasEl?.getBoundingClientRect();
      if (!rect || rect.width === 0 || rect.height === 0) return { x: 0, y: 0 };
      return {
        x: ((event.clientX - rect.left) / rect.width) * app.screen.width,
        y: ((event.clientY - rect.top) / rect.height) * app.screen.height,
      };
    };

    const zoomAt = (anchorX: number, factor: number) => {
      const width = app.screen.width;
      const current = domainRef.current;
      const limits = absoluteRef.current;
      const anchorYear = xToYear(anchorX, current, width);
      const duration = clamp(current.end - current.start, 1, limits.end - limits.start);
      const nextDuration = clamp(duration * factor, 1, limits.end - limits.start);
      const ratio = anchorX / Math.max(1, width);
      let start = anchorYear - nextDuration * ratio;
      let end = start + nextDuration;
      if (start < limits.start) {
        end += limits.start - start;
        start = limits.start;
      }
      if (end > limits.end) {
        start -= end - limits.end;
        end = limits.end;
      }
      domainRef.current = {
        start: clamp(start, limits.start, limits.end - 1),
        end: clamp(end, limits.start + 1, limits.end),
      };
      applyView();
      publishDomain();
      scheduleZoomCommit();
    };

    const onPointerMove = (event: PointerEvent) => {
      if (!ready || destroyed || !canvasEl) return;
      const point = localPoint(event);
      if (pointers.has(event.pointerId)) pointers.set(event.pointerId, point);
      if (pointers.size === 2) {
        const [first, second] = [...pointers.values()];
        const distance = Math.hypot(first.x - second.x, first.y - second.y);
        if (pinchDistance > 0 && distance > 0) {
          zoomAt((first.x + second.x) / 2, pinchDistance / distance);
        }
        pinchDistance = distance;
        return;
      }
      pointerX = point.x;
      pointerY = point.y;
      if (dragging && dragDomain) {
        if (Math.abs(point.x - dragStartX) > 4) dragMoved = true;
        const width = app.screen.width;
        const duration = dragDomain.end - dragDomain.start;
        const yearDelta = -((point.x - dragStartX) / width) * duration;
        const limits = absoluteRef.current;
        let start = dragDomain.start + yearDelta;
        let end = dragDomain.end + yearDelta;
        if (start < limits.start) {
          end += limits.start - start;
          start = limits.start;
        }
        if (end > limits.end) {
          start -= end - limits.end;
          end = limits.end;
        }
        domainRef.current = { start, end };
        applyView();
        publishDomain();
        return;
      }
      currentHovered = findNearest(pointerX, pointerY);
      const hoveredPoint = currentHovered;
      canvasEl.style.cursor = hoveredPoint ? "pointer" : "grab";
      if (hoveredPoint) {
        setHovered((previous) => previous?.id === hoveredPoint.id ? previous : hoveredPoint.event);
        setHoverPosition({ x: hoveredPoint.x, y: hoveredPoint.y });
      } else {
        setHovered(null);
      }
      draw();
    };

    const onPointerDown = (event: PointerEvent) => {
      if (!ready || destroyed) return;
      const point = localPoint(event);
      pointers.set(event.pointerId, point);
      if (pointers.size === 2) {
        dragging = false;
        const [first, second] = [...pointers.values()];
        pinchDistance = Math.hypot(first.x - second.x, first.y - second.y);
        return;
      }
      const nearest = findNearest(point.x, point.y);
      downId = nearest?.id ?? null;
      dragMoved = false;
      if (!nearest) {
        dragging = true;
        dragStartX = point.x;
        dragDomain = { ...domainRef.current };
        if (canvasEl) canvasEl.style.cursor = "grabbing";
      }
    };

    const onPointerUp = (event: PointerEvent) => {
      if (!ready || destroyed) return;
      pointers.delete(event.pointerId);
      if (pointers.size < 2) pinchDistance = 0;
      if (dragging) {
        dragging = false;
        dragDomain = null;
        if (dragMoved) {
          window.clearTimeout(commitTimer.current);
          const [start, end] = domainToYearWindow(domainRef.current);
          onZoomRef.current(start, end);
        }
      } else if (downId && !dragMoved) {
        onSelectRef.current(downId);
      }
      downId = null;
      if (canvasEl) canvasEl.style.cursor = "grab";
    };

    const onWheel = (event: WheelEvent) => {
      if (!ready || destroyed) return;
      event.preventDefault();
      const point = localPoint(event as unknown as PointerEvent);
      const factor = event.deltaY > 0 ? 1.12 : 0.88;
      zoomAt(point.x, factor);
    };

    const tick = () => {
      if (!ready || destroyed) return;
      const reduced = reduceMotion();
      let moving = false;
      const kept: Particle[] = [];
      for (const point of particles) {
        if (reduced) {
          point.x = point.targetX;
          point.y = point.baseY;
          point.alpha = point.targetAlpha;
        } else {
          const dx = pointerX - point.x;
          const dy = pointerY - point.y;
          const distance = Math.hypot(dx, dy);
          let repulse = 0;
          if (!dragging && distance > 0 && distance < 72 && point.id !== currentHovered?.id) {
            repulse = (-dy / distance) * (1 - distance / 72) * 8;
          }
          const targetY = point.baseY + repulse;
          const stepX = point.targetX - point.x;
          const stepY = targetY - point.y;
          const stepA = point.targetAlpha - point.alpha;
          if (Math.abs(stepX) > 0.2) {
            point.x += stepX * 0.18;
            moving = true;
          } else point.x = point.targetX;
          if (Math.abs(stepY) > 0.2) {
            point.y += stepY * 0.2;
            moving = true;
          } else point.y = targetY;
          if (Math.abs(stepA) > 0.02) {
            point.alpha += stepA * 0.16;
            moving = true;
          } else point.alpha = point.targetAlpha;
        }
        if (point.targetAlpha > 0 || point.alpha > 0.03) kept.push(point);
      }
      if (kept.length !== particles.length) {
        particles = kept;
        moving = true;
      }
      if (moving) {
        rebuildBuckets();
        draw();
      }
    };

    const teardown = () => {
      if (tornDown) return;
      tornDown = true;
      ready = false;
      if (pixiAppRef.current === app) pixiAppRef.current = null;
      if (syncSceneRef.current === placeAll) syncSceneRef.current = () => {};
      if (drawSceneRef.current === draw) drawSceneRef.current = () => {};
      app.ticker?.remove(tick);
      canvasEl?.removeEventListener("wheel", onWheel);
      canvasEl?.removeEventListener("pointerdown", onPointerDown);
      canvasEl?.removeEventListener("pointermove", onPointerMove);
      canvasEl?.removeEventListener("pointerup", onPointerUp);
      canvasEl?.removeEventListener("pointercancel", onPointerUp);
      canvasEl = undefined;
      try {
        app.destroy(true, { children: true });
      } catch {
        // Pixi may already be torn down during React StrictMode's second mount.
      }
    };

    const init = async () => {
      await app.init({
        resizeTo: host,
        background: aegeanPixiColors(pixiTheme()).background,
        antialias: true,
        autoDensity: true,
        resolution: Math.min(window.devicePixelRatio || 1, 2),
        powerPreference: "high-performance",
      });
      if (destroyed) {
        teardown();
        return;
      }
      canvasEl = app.canvas;
      canvasEl.style.touchAction = "none";
      pixiAppRef.current = app;
      host.appendChild(canvasEl);
      stage.addChild(dots);
      stage.addChild(guides);
      app.stage.addChild(stage);
      ready = true;
      pixiReadyRef.current = true;
      syncSceneRef.current = placeAll;
      drawSceneRef.current = draw;
      const fit = () => {
        if (destroyed) return;
        const width = host.clientWidth;
        const height = host.clientHeight;
        if (width > 0 && height > 0 && app.renderer) app.renderer.resize(width, height);
        placeAll();
      };
      fit();
      requestAnimationFrame(fit);
      canvasEl.addEventListener("wheel", onWheel, { passive: false });
      canvasEl.addEventListener("pointerdown", onPointerDown);
      canvasEl.addEventListener("pointermove", onPointerMove);
      canvasEl.addEventListener("pointerup", onPointerUp);
      canvasEl.addEventListener("pointercancel", onPointerUp);
      app.ticker.add(tick);
    };

    const resizeObserver = new ResizeObserver(() => {
      if (!ready || destroyed) return;
      const width = host.clientWidth;
      const height = host.clientHeight;
      if (width > 0 && height > 0 && app.renderer) app.renderer.resize(width, height);
      placeAll();
    });
    resizeObserver.observe(host);
    const initPromise = init().catch(() => {});

    return () => {
      destroyed = true;
      pixiReadyRef.current = false;
      cancelAnimationFrame(publishFrame.current);
      window.clearTimeout(commitTimer.current);
      resizeObserver.disconnect();
      void initPromise.then(teardown);
    };
  }, []);

  useEffect(() => {
    const app = pixiAppRef.current;
    if (!app?.renderer) return;
    app.renderer.background.color = aegeanPixiColors(pixiThemeFromDom(theme)).background;
    drawSceneRef.current();
  }, [theme]);

  const histogram = useMemo(() => {
    const bins = Array<number>(64).fill(0);
    const span = Math.max(1, absoluteDomain.end - absoluteDomain.start);
    for (const event of datedEvents) {
      const year = eventYear(event);
      if (year === null) continue;
      const ratio = (year - absoluteDomain.start) / span;
      bins[Math.min(63, Math.max(0, Math.floor(ratio * 64)))]++;
    }
    return bins;
  }, [datedEvents, absoluteDomain]);

  const yearWindow = domainToYearWindow(domain);
  const visibleCount = filterEventsByLod(datedEvents, domain).length;
  const selected = events.find((event) => event.id === spotlightEventId);
  const ticks = axisTicks(domain);
  const hostWidth = canvasHostRef.current?.clientWidth ?? 800;
  const tooltipLeft = clamp(hoverPosition.x, 120, Math.max(140, hostWidth - 120));
  const tooltipAbove = hoverPosition.y > 96;

  function onKeyDown(event: KeyboardEvent<HTMLElement>) {
    const span = domain.end - domain.start;
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      applyDomain({ start: domain.start - span * 0.15, end: domain.end - span * 0.15 }, true);
    } else if (event.key === "ArrowRight") {
      event.preventDefault();
      applyDomain({ start: domain.start + span * 0.15, end: domain.end + span * 0.15 }, true);
    } else if (event.key === "+" || event.key === "=") {
      event.preventDefault();
      const next = span * 0.8;
      const mid = (domain.start + domain.end) / 2;
      applyDomain({ start: mid - next / 2, end: mid + next / 2 }, true);
    } else if (event.key === "-" || event.key === "_") {
      event.preventDefault();
      const next = span * 1.25;
      const mid = (domain.start + domain.end) / 2;
      applyDomain({ start: mid - next / 2, end: mid + next / 2 }, true);
    }
  }

  return (
    <section
      className={`historical-timeline${mode === "fullscreen" ? " historical-timeline--fullscreen" : ""}`}
      aria-label="Interactive timeline"
      data-mode={mode}
      tabIndex={0}
      onKeyDown={onKeyDown}
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
            {t.timelineAllMoments} <span>{events.length}</span>
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
            <span>{formatYearLabel(domain.start, locale)}</span>
            <span className="historical-timeline__header-label">
              {visibleCount < datedEvents.length
                ? `${visibleCount}/${datedEvents.length}`
                : t.lifeInTime}
            </span>
            <span>{formatYearLabel(domain.end, locale)}</span>
          </div>
          <div className="historical-timeline__axis" aria-hidden="true">
            {ticks.map((tick) => {
              const ratio = (tick - domain.start) / Math.max(1e-6, domain.end - domain.start);
              return (
                <span key={tick} style={{ left: `${ratio * 100}%` }}>
                  {formatYearLabel(tick, locale)}
                </span>
              );
            })}
          </div>
          {hovered ? (
            <div
              className={`historical-timeline__tooltip${tooltipAbove ? "" : " is-below"}`}
              style={{ left: tooltipLeft, top: hoverPosition.y }}
            >
              <div className="historical-timeline__tooltip-year">{eventDate(hovered)}</div>
              <div className="historical-timeline__tooltip-title">{hovered.title}</div>
              <div className="historical-timeline__tooltip-meta">
                {legendLabel(legendKeyForEventType(hovered.event_type), locale)}
              </div>
            </div>
          ) : null}
          {selected ? (
            <article className="historical-timeline__card">
              <p className="historical-timeline__card-year">{eventDate(selected)}</p>
              <h3>{selected.title}</h3>
              <p>{legendLabel(legendKeyForEventType(selected.event_type), locale)}</p>
              {selected.place_label ? <p>{selected.place_label}</p> : null}
              {selected.time?.kind === "approx" || selected.time?.kind === "unknown" ? (
                <p className="historical-timeline__card-note">{t.timelineApproximate}</p>
              ) : null}
              <button type="button" onClick={() => onSelect(selected.id)}>
                {t.timelineOpenDetail}
              </button>
            </article>
          ) : null}
        </div>
      </div>
      <footer className="historical-timeline__footer">
        <p className="historical-timeline__period">{t.visiblePeriod(formatYearLabel(domain.start, locale), formatYearLabel(domain.end, locale))}</p>
        {coverage ? (
          <p className="historical-timeline__coverage">{t.timelineMapCoverage(coverage.timeline, coverage.mapped)}</p>
        ) : null}
        <div className="historical-timeline__phases" role="group" aria-label={t.timelinePhases}>
          {phases.map((phase) => (
            <button
              key={phase.id}
              type="button"
              onClick={() => applyDomain(yearWindowToDomain(phase.window), true)}
            >
              {phaseLabel(phase.id, phase.label, phase.documented)}
            </button>
          ))}
        </div>
        <HistographyBrush
          bounds={bounds}
          window={yearWindow}
          histogram={histogram}
          onChange={(next) => applyDomain(yearWindowToDomain(next), false)}
          onCommit={(next) => applyDomain(yearWindowToDomain(next), true)}
        />
      </footer>
    </section>
  );
}
