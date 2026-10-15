// web/src/lib/chronological-preview.test.ts
import { expect, test } from "vitest";
import type { TimelineEvent } from "@/lib/api";
import { previewImportanceScore, selectChronologicalPreview } from "@/lib/chronological-preview";

function ev(
  partial: Partial<TimelineEvent> & Pick<TimelineEvent, "id" | "event_type" | "title">,
): TimelineEvent {
  return {
    entity_id: "e1",
    person: "N",
    epistemic_status: "attested",
    map_eligible: true,
    time: { kind: "approx", start: "1800", precision: "year" },
    ...partial,
  };
}

test("prefers battles and office over long anecdotes", () => {
  const battle = ev({ id: "b", event_type: "battle", title: "Battle of Austerlitz" });
  const anecdote = ev({
    id: "a",
    event_type: "anecdote",
    title: "A very long narrative sentence that should not dominate the overview preview list",
  });
  expect(previewImportanceScore(battle)).toBeGreaterThan(previewImportanceScore(anecdote));
});

test("selectChronologicalPreview picks significant acts and sorts by year", () => {
  const events = [
    ev({ id: "1", event_type: "birth", title: "Born", time: { kind: "exact", start: "1769", precision: "year" } }),
    ev({ id: "2", event_type: "travel", title: "Trip", time: { kind: "approx", start: "1770", precision: "year" } }),
    ev({ id: "3", event_type: "battle", title: "Austerlitz", time: { kind: "approx", start: "1805", precision: "year" } }),
    ev({ id: "4", event_type: "office", title: "Consul", time: { kind: "approx", start: "1799", precision: "year" } }),
    ev({ id: "5", event_type: "death", title: "Died", time: { kind: "exact", start: "1821", precision: "year" } }),
  ];
  const preview = selectChronologicalPreview(events, 4);
  expect(preview.map((e) => e.id)).toEqual(["2", "4", "3"]);
});

test("dense biographies do not elevate births and deaths over major events", () => {
  const events = [
    ev({ id: "birth", event_type: "birth", title: "Birth" }),
    ev({ id: "death", event_type: "death", title: "Death" }),
    ev({ id: "work", event_type: "publication", title: "Publication" }),
    ev({ id: "battle", event_type: "battle", title: "Austerlitz" }),
    ev({ id: "office", event_type: "office", title: "Coronation" }),
  ];
  expect(selectChronologicalPreview(events, 2).map(e => e.id)).toEqual(["battle", "office"]);
});
