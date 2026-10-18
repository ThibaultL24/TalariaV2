// web/src/components/timeline/timeline-utils.test.ts
import { expect, test } from "vitest";
import type { TimelineEvent } from "@/lib/api";
import { filterEventsByLod, separateStackedPoints } from "./timeline-utils";

function event(id: string, type: string): TimelineEvent {
  return {
    id,
    entity_id: "person",
    person: "Person",
    event_type: type,
    epistemic_status: "verified",
    title: id,
    time: { kind: "exact", start: "1800", precision: "year" },
    map_eligible: false,
    confidence: 0.5,
  };
}

test("a wide period keeps the important events and hides the minor ones", () => {
  const birth = { ...event("birth", "birth"), map_eligible: true, confidence: 0.95 };
  const note = { ...event("note", "anecdote"), confidence: 0.4 };
  const visible = filterEventsByLod([birth, note], { start: 1000, end: 7000 });
  expect(visible.map((item) => item.id)).toEqual(["birth"]);
});

test("points that share a year are pushed apart vertically", () => {
  const placed = separateStackedPoints([
    { id: "a", x: 10, y: 40 },
    { id: "b", x: 12, y: 40 },
  ]);
  expect(placed[0].y).toBe(40);
  expect(placed[1].y).toBeGreaterThan(placed[0].y + 8);
});
