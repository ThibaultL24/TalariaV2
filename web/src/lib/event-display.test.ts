// web/src/lib/event-display.test.ts
import { describe, expect, test } from "vitest";
import type { TimelineEvent } from "@/lib/api";
import { eventDate } from "@/lib/entity-views";
import { localizedEventTitle } from "@/lib/localize-event-copy";

function battle(partial: Partial<TimelineEvent> = {}): TimelineEvent {
  return {
    id: "mondovi",
    entity_id: "napoleon",
    person: "Napoleon Bonaparte",
    event_type: "battle",
    epistemic_status: "attested",
    title: "Battle of Mondovì",
    map_eligible: true,
    time: {
      kind: "exact",
      precision: "day",
      start: "1796-04-21",
      surface: "21 April 1796",
    },
    place_label: "Mondovì",
    ...partial,
  };
}

describe("event display keeps the date out of the title", () => {
  test("a French frieze title names the battle and the town", () => {
    expect(localizedEventTitle(battle(), "fr")).toBe("Bataille · Mondovì");
  });

  test("the frieze date follows the typed day", () => {
    expect(eventDate(battle(), "fr")).toBe("21 avril 1796");
    expect(eventDate(battle(), "en")).toBe("21 April 1796");
  });
});
