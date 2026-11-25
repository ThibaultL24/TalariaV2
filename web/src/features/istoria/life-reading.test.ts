// web/src/features/istoria/life-reading.test.ts
import { expect, test } from "vitest";
import type { TimelineEvent } from "@/lib/api";
import { compareEvents, eventYear, eventsInBand, lifeBands, successionSegments } from "./life-reading";

function event(partial: Partial<TimelineEvent> & Pick<TimelineEvent, "id" | "title">): TimelineEvent {
  return {
    entity_id: "person",
    person: "Napoléon",
    event_type: "battle",
    epistemic_status: "accepted",
    map_eligible: true,
    ...partial,
  };
}

test("orders a range and an approximate year before an undated fact", () => {
  const undated = event({ id: "later", title: "Sans date" });
  const ranged = event({
    id: "range",
    title: "Campagne",
    time: { kind: "range", precision: "year", start: "1796-04-01", end: "1797-10-17" },
  });
  const approx = event({
    id: "approx",
    title: "Ajaccio",
    time: { kind: "approx", precision: "year", start: "1769-01-01" },
  });
  const ordered = [undated, ranged, approx].sort(compareEvents);
  expect(ordered.map((item) => item.id)).toEqual(["approx", "range", "later"]);
  expect(eventYear(undated)).toBeNull();
});

test("keeps simultaneous facts and breaks the line when a place is missing", () => {
  const first = event({
    id: "a",
    title: "A",
    time: { kind: "exact", precision: "day", start: "1805-12-02" },
    coordinates: { lat: 49.1, lon: 16.7 },
  });
  const sameDay = event({
    id: "b",
    title: "B",
    time: { kind: "exact", precision: "day", start: "1805-12-02" },
    coordinates: { lat: 48.8, lon: 16.6 },
  });
  const unplaced = event({
    id: "c",
    title: "C",
    time: { kind: "exact", precision: "year", start: "1806-01-01" },
    map_eligible: false,
  });
  const next = event({
    id: "d",
    title: "D",
    time: { kind: "exact", precision: "year", start: "1806-10-14" },
    coordinates: { lat: 50.9, lon: 11.6 },
  });
  const ordered = [next, sameDay, unplaced, first].sort(compareEvents);
  expect(ordered.map((item) => item.id)).toEqual(["a", "b", "c", "d"]);
  expect(successionSegments(ordered)).toEqual([
    [[16.7, 49.1], [16.6, 48.8]],
  ]);
  const bands = lifeBands(ordered);
  expect(bands[0]).toMatchObject({ from: 1800, to: 1809 });
  expect(eventsInBand(ordered, bands[0])).toHaveLength(4);
});
