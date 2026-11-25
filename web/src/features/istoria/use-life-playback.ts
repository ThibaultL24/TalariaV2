// web/src/features/istoria/use-life-playback.ts
import { useEffect, useMemo, useState } from "react";
import type { TimelineEvent } from "@/lib/api";
import {
  compareEvents,
  eventPoint,
  eventsInBand,
  lifeBands,
  playbackDelay,
  successionSegments,
  type LifeBand,
  type LifeSpeed,
} from "./life-reading";

interface LifePlaybackOptions {
  events: TimelineEvent[];
  selectedId?: string;
  onSelect: (id: string) => void;
}

export function useLifePlayback({ events, selectedId, onSelect }: LifePlaybackOptions) {
  const [playing, setPlaying] = useState(false);
  const [showAll, setShowAll] = useState(true);
  const [speed, setSpeed] = useState<LifeSpeed>(1);
  const [bandId, setBandId] = useState<string | null>(null);
  const [index, setIndex] = useState(0);
  const ordered = useMemo(() => [...events].sort(compareEvents), [events]);
  const bands = useMemo(() => lifeBands(ordered), [ordered]);
  const band = bands.find((item) => item.id === bandId) ?? null;
  const scoped = useMemo(() => eventsInBand(ordered, band), [ordered, band]);
  const last = Math.max(0, scoped.length - 1);
  const safeIndex = Math.min(index, last);
  const active = scoped[safeIndex];
  const revealed = useMemo(
    () => (showAll ? scoped : scoped.slice(0, safeIndex + 1)),
    [showAll, scoped, safeIndex],
  );
  const trail = useMemo(() => successionSegments(revealed), [revealed]);
  const undated = ordered.length - eventsInBand(ordered, null).length;

  useEffect(() => {
    if (!playing || showAll || scoped.length === 0) return;
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const delay = playbackDelay(speed, reduced);
    if (delay === 0) {
      setPlaying(false);
      return;
    }
    const timer = window.setInterval(() => {
      setIndex((current) => {
        if (current >= scoped.length - 1) {
          setPlaying(false);
          return scoped.length - 1;
        }
        return current + 1;
      });
    }, delay);
    return () => window.clearInterval(timer);
  }, [playing, showAll, speed, scoped.length]);

  useEffect(() => {
    if (showAll || !active || active.id === selectedId) return;
    onSelect(active.id);
  }, [showAll, active, selectedId, onSelect]);

  useEffect(() => {
    if (playing || showAll || !selectedId) return;
    const found = scoped.findIndex((event) => event.id === selectedId);
    if (found >= 0 && found !== index) setIndex(found);
  }, [selectedId, playing, showAll, scoped, index]);

  const point = active ? eventPoint(active) : null;

  function play() {
    setShowAll(false);
    if (selectedId) {
      const found = scoped.findIndex((event) => event.id === selectedId);
      setIndex(found >= 0 ? found : 0);
    }
    setPlaying(true);
  }

  function pause() {
    setPlaying(false);
  }

  function step(delta: number) {
    setShowAll(false);
    setPlaying(false);
    setIndex((current) => Math.min(last, Math.max(0, current + delta)));
  }

  function restart() {
    setShowAll(false);
    setPlaying(false);
    setIndex(0);
  }

  function showEveryEvent() {
    setPlaying(false);
    setShowAll(true);
  }

  function chooseBand(next: LifeBand | null) {
    setBandId(next?.id ?? null);
    setIndex(0);
    setShowAll(false);
    setPlaying(false);
  }

  return {
    playing,
    showAll,
    speed,
    setSpeed,
    bands,
    band,
    scoped,
    index: safeIndex,
    active,
    revealed,
    trail,
    undated,
    point,
    play,
    pause,
    step,
    restart,
    showEveryEvent,
    chooseBand,
  };
}
