// web/src/lib/chronological-preview.ts
import type { TimelineEvent } from "@/lib/api";

const MAJOR_TYPE_SCORE: Record<string, number> = {
  birth: 12,
  death: 12,
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
  residence: 25,
  education: 30,
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
  if (event.event_type === "birth" || event.event_type === "death") score -= 20;
  return score;
}

/**
 * Pick meaningful life episodes; birth/death are fallback only.
 * Result is sorted chronologically for reading.
 */
export function selectChronologicalPreview(
  events: TimelineEvent[],
  limit = 10,
): TimelineEvent[] {

  // Landmark status must be earned by sourced significance, not just chronology.
  // Birth and death belong in biographical metadata rather than monopolizing cards.
  const ranked = [...events]
    .filter((event) => !["birth", "death"].includes(event.event_type))
    .sort(
      (a, b) =>
        previewImportanceScore(b) - previewImportanceScore(a) ||
        compareChronological(a, b),
    )
    .slice(0, limit);

  const picked = ranked.length
    ? ranked
    : [...events].sort((a, b) => previewImportanceScore(b) - previewImportanceScore(a)).slice(0, limit);

  return picked.sort(compareChronological);
}
