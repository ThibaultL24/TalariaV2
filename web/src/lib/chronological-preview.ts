// web/src/lib/chronological-preview.ts
import type { TimelineEvent } from "@/lib/api";

const MAJOR_TYPE_SCORE: Record<string, number> = {
  birth: 100,
  death: 100,
  battle: 88,
  siege: 84,
  office: 86,
  diplomatic: 78,
  treaty: 76,
  marriage: 74,
  exile: 80,
  imprisonment: 72,
  meeting: 68,
  speech: 66,
  award: 64,
  publication: 58,
  creation: 56,
  travel: 42,
  residence: 38,
  education: 40,
  anecdote: 28,
  historical_fact: 32,
  life_event: 30,
};

function eventYear(event: TimelineEvent): number | null {
  const start = event.time?.start ?? event.start_time ?? null;
  if (!start || event.time?.kind === "unknown") return null;
  const match = String(start).match(/^(-?\d{4,})/);
  return match ? Number(match[1]) : null;
}

function compareChronological(a: TimelineEvent, b: TimelineEvent): number {
  const ya = eventYear(a);
  const yb = eventYear(b);
  if (ya === null && yb === null) return a.title.localeCompare(b.title);
  if (ya === null) return 1;
  if (yb === null) return -1;
  if (ya !== yb) return ya - yb;
  return a.title.localeCompare(b.title);
}

/** Prefer explorer headlines over long extracted sentences. */
function titleQualityPenalty(title: string): number {
  const len = title.trim().length;
  if (len > 140) return 35;
  if (len > 90) return 20;
  if (len > 60) return 8;
  return 0;
}

export function previewImportanceScore(event: TimelineEvent): number {
  let score = MAJOR_TYPE_SCORE[event.event_type] ?? 45;
  score -= titleQualityPenalty(event.title);
  if (event.epistemic_status === "established") score += 6;
  if (event.time?.precision === "day") score += 10;
  else if (event.time?.precision === "month") score += 5;
  if (event.map_eligible) score += 3;
  return score;
}

/**
 * Pick landmark moments for the overview list: birth/death anchors + top-ranked public life.
 * Result is sorted chronologically for reading.
 */
export function selectChronologicalPreview(
  events: TimelineEvent[],
  limit = 10,
): TimelineEvent[] {
  if (events.length <= limit) {
    return [...events].sort(compareChronological);
  }

  const byId = new Map(events.map((event) => [event.id, event]));
  const birth = events.find((event) => event.event_type === "birth");
  const death = events.find((event) => event.event_type === "death");
  const reserved = new Set<string>();
  if (birth) reserved.add(birth.id);
  if (death) reserved.add(death.id);

  const slots = Math.max(1, limit - reserved.size);
  const ranked = events
    .filter((event) => !reserved.has(event.id))
    .sort(
      (a, b) =>
        previewImportanceScore(b) - previewImportanceScore(a) ||
        compareChronological(a, b),
    )
    .slice(0, slots);

  const picked: TimelineEvent[] = [];
  for (const id of [birth?.id, ...ranked.map((e) => e.id), death?.id]) {
    if (!id || picked.some((e) => e.id === id)) continue;
    const event = byId.get(id);
    if (event) picked.push(event);
  }

  return picked.sort(compareChronological);
}
