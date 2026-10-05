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

test("selectChronologicalPreview keeps birth and death and sorts by year", () => {
  const events = [
    ev({ id: "1", event_type: "birth", title: "Born", time: { kind: "exact", start: "1769", precision: "year" } }),
    ev({ id: "2", event_type: "travel", title: "Trip", time: { kind: "approx", start: "1770", precision: "year" } }),
    ev({ id: "3", event_type: "battle", title: "Austerlitz", time: { kind: "approx", start: "1805", precision: "year" } }),
    ev({ id: "4", event_type: "office", title: "Consul", time: { kind: "approx", start: "1799", precision: "year" } }),
    ev({ id: "5", event_type: "death", title: "Died", time: { kind: "exact", start: "1821", precision: "year" } }),
  ];
  const preview = selectChronologicalPreview(events, 4);
  expect(preview.map((e) => e.id)).toEqual(["1", "4", "3", "5"]);
});
