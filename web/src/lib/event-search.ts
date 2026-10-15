// web/src/lib/event-search.ts
import type { TimelineEvent } from "@/lib/api";

export function normalizeEventQuery(query: string): string {
  return query
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .toLowerCase()
    .trim();
}

function scoreField(haystack: string, needle: string): number {
  const h = normalizeEventQuery(haystack);
  if (!h || !needle) return 0;
  if (h === needle) return 100;
  if (h.startsWith(needle)) return 80;
  if (h.includes(needle)) return 55;
  const tokens = needle.split(/\s+/).filter(Boolean);
  if (tokens.length > 1 && tokens.every((t) => h.includes(t))) return 48;
  return 0;
}

export function scoreEventMatch(event: TimelineEvent, query: string): number {
  const needle = normalizeEventQuery(query);
  if (needle.length < 2) return 0;
  const scores = [
    scoreField(event.title, needle) * 1.2,
    scoreField(event.summary ?? "", needle),
    scoreField(event.place_label ?? "", needle) * 1.05,
    scoreField(event.event_type.replaceAll("_", " "), needle) * 0.6,
  ];
  return Math.max(...scores);
}

export function searchEvents(
  events: TimelineEvent[],
  query: string,
  limit = 10,
): TimelineEvent[] {
  const needle = normalizeEventQuery(query);
  if (needle.length < 2) return [];
  return events
    .map((event) => ({ event, score: scoreEventMatch(event, needle) }))
    .filter((row) => row.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, limit)
    .map((row) => row.event);
}
