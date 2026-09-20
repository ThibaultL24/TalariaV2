// sidecar/intuition/schemas.test.ts
import { describe, expect, it } from "vitest";
import {
  SCHEMA_VERSION_V2,
  SCHEMA_VERSION_V3,
  parseDebateFactInput,
  type DebateFact,
} from "./schemas.ts";

const v2Fact = {
  version: SCHEMA_VERSION_V2,
  debate_id: "talaria:debate:napoleon-death:poison",
  kind: "theory",
  question: { text: "How did Napoleon die?" },
  proposition: { text: "Napoleon was poisoned on Saint Helena" },
  about_event: {
    canonical_event_id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
    title: "Death of Napoleon",
    event_type: "death",
    time_surface: "1821-05-05",
  },
};

describe("parseDebateFactInput", () => {
  it("accepts v2 snake_case DebateFact", () => {
    const fact = parseDebateFactInput(v2Fact);
    expect(fact.version).toBe(SCHEMA_VERSION_V2);
    expect(fact.debate_id).toContain("napoleon-death");
    expect(fact.about_event?.time_surface).toBe("1821-05-05");
  });

  it("accepts v3 camelCase envelope and normalizes to DebateFact", () => {
    const fact = parseDebateFactInput({
      version: SCHEMA_VERSION_V3,
      publicationId: "11111111-2222-3333-4444-555555555555",
      network: "testnet",
      fact: {
        kind: "theory",
        question: { text: "How did Napoleon die?" },
        proposition: { text: "Napoleon was poisoned on Saint Helena" },
        aboutEvent: {
          id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
          type: "death",
          time: {
            kind: "exact",
            start: "1821-05-05",
            precision: "day",
            calendar: "gregorian",
          },
        },
      },
    });
    expect(fact.kind).toBe("theory");
    expect(fact.about_event?.canonical_event_id).toBe(
      "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
    );
    expect(fact.about_event?.time_surface).toBe("1821-05-05");
  });

  it("rejects empty proposition", () => {
    expect(() =>
      parseDebateFactInput({
        ...v2Fact,
        proposition: { text: "  " },
      }),
    ).toThrow(/proposition/i);
  });

  it("year-only time in v3 does not become YYYY-01-01", () => {
    const fact = parseDebateFactInput({
      version: SCHEMA_VERSION_V3,
      publicationId: "11111111-2222-3333-4444-555555555555",
      network: "testnet",
      fact: {
        kind: "place_conflict",
        question: { text: "Where was the battle?" },
        proposition: { text: "Austerlitz" },
        aboutEvent: {
          id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
          type: "battle",
          time: { kind: "exact", start: "1805", precision: "year" },
        },
      },
    });
    expect(fact.about_event?.time_surface).toBe("1805");
    expect(fact.about_event?.time_surface).not.toMatch(/1805-01-01/);
  });
});

describe("DebateFact type export", () => {
  it("keeps a stable shape for the model layer", () => {
    const fact: DebateFact = parseDebateFactInput(v2Fact);
    expect(fact.question.text.length).toBeGreaterThan(0);
  });
});
