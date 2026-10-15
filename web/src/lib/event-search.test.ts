import { describe, expect, it } from "vitest";
import type { TimelineEvent } from "@/lib/api";
import { searchEvents } from "./event-search";

function event(title: string, extra: Partial<TimelineEvent> = {}): TimelineEvent {
  return {
    id: title,
    entity_id: "p1",
    person: "Napoleon",
    event_type: "battle",
    epistemic_status: "documented",
    title,
    map_eligible: true,
    ...extra,
  };
}

describe("searchEvents", () => {
  it("matches title and place", () => {
    const events = [
      event("Battle of Austerlitz", { place_label: "Austerlitz" }),
      event("Coronation at Notre-Dame"),
    ];
    const hits = searchEvents(events, "austerlitz");
    expect(hits[0]?.title).toContain("Austerlitz");
  });
});
