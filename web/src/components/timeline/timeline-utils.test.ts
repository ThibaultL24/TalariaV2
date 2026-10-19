// web/src/components/timeline/timeline-utils.test.ts
import { expect, test } from "vitest";
import type { TimelineEvent } from "@/lib/api";
import {
  eventImportance,
  eventTimeValue,
  filterEventsByLod,
  formatYearLabel,
  layoutConstellation,
  lifePhasePresets,
  separateStackedPoints,
} from "./timeline-utils";

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

test("a life-sized window keeps every dated event", () => {
  const events = Array.from({ length: 400 }, (_, index) => ({
    ...event(`note-${index}`, "anecdote"),
    confidence: 0.4,
  }));
  expect(filterEventsByLod(events, { start: 1769, end: 1870 })).toHaveLength(400);
});

test("a wide period keeps the important events and hides the minor ones", () => {
  const birth = { ...event("birth", "birth"), map_eligible: true, confidence: 0.95 };
  const note = { ...event("note", "anecdote"), confidence: 0.4 };
  const visible = filterEventsByLod([birth, note], { start: 1000, end: 7000 });
  expect(visible.map((item) => item.id)).toEqual(["birth"]);
});

test("a map pin does not raise historical importance", () => {
  const plain = event("a", "battle");
  const mapped = { ...plain, id: "b", map_eligible: true };
  expect(eventImportance(mapped)).toBe(eventImportance(plain));
});

test("constellation positions stay put and follow event families", () => {
  const birth = { ...event("birth", "birth"), time: { kind: "exact" as const, start: "1802", precision: "year" as const } };
  const battle = { ...event("battle", "battle"), time: { kind: "exact" as const, start: "1815", precision: "year" as const } };
  const domain = { start: 1800, end: 1830 };
  const first = layoutConstellation([birth, battle], domain, 900, 600);
  const second = layoutConstellation([birth, battle], domain, 900, 600);
  expect(first.map((point) => [point.event.id, point.x, point.y])).toEqual(
    second.map((point) => [point.event.id, point.x, point.y]),
  );
  const birthY = first.find((point) => point.event.id === "birth")?.y ?? 0;
  const battleY = first.find((point) => point.event.id === "battle")?.y ?? 0;
  expect(birthY).toBeLessThan(battleY);
});

test("three thousand events keep a stable layout quickly", () => {
  const events = Array.from({ length: 3000 }, (_, index) => ({
    ...event(`event-${index}`, index % 11 === 0 ? "battle" : "anecdote"),
    time: { kind: "exact" as const, start: String(1700 + (index % 120)), precision: "year" as const },
  }));
  const started = performance.now();
  const placed = layoutConstellation(events, { start: 1700, end: 1820 }, 1400, 800);
  expect(performance.now() - started).toBeLessThan(120);
  expect(placed.length).toBe(3000);
  expect(layoutConstellation(events, { start: 1700, end: 1820 }, 1400, 800).map((point) => point.y))
    .toEqual(placed.map((point) => point.y));
});

test("negative years and months stay on the right side of the axis", () => {
  expect(formatYearLabel(-44, "en")).toBe("44 BCE");
  expect(formatYearLabel(-44, "fr")).toContain("av.");
  const dated = event("bce", "birth");
  dated.time = { kind: "exact", start: "-0044-06-15", precision: "day" };
  expect(eventTimeValue(dated)).toBeGreaterThan(-44);
  expect(eventTimeValue(dated)).toBeLessThan(-43);
});

test("documented life periods replace the generic thirds", () => {
  const generic = lifePhasePresets([1800, 1890]);
  expect(generic.every((phase) => phase.documented === false)).toBe(true);
  expect(generic.map((phase) => phase.id)).toEqual(["full", "opening", "middle", "closing"]);
  const documented = lifePhasePresets([1800, 1890], [
    { id: "exile", label: "Exile", window: [1851, 1870] },
  ]);
  expect(documented.map((phase) => phase.id)).toEqual(["full", "exile"]);
  expect(documented[1]?.documented).toBe(true);
});

test("points that share a year are pushed apart vertically", () => {
  const placed = separateStackedPoints([
    { id: "a", x: 10, y: 40 },
    { id: "b", x: 12, y: 40 },
  ]);
  expect(placed[0].y).toBe(40);
  expect(placed[1].y).toBeGreaterThan(placed[0].y + 8);
});
