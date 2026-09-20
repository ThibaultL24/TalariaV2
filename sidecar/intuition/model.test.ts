// sidecar/intuition/model.test.ts
import { describe, expect, it } from "vitest";
import { modelDebate, startDateField } from "./model.ts";
import { SCHEMA_VERSION_V2 } from "./schemas.ts";
import { atomDataFromPinUri } from "./pin.ts";

const fact = {
  version: SCHEMA_VERSION_V2,
  debate_id: "talaria:debate:napoleon-battle-1805:at-austerlitz",
  kind: "place_conflict",
  question: { text: "Where was Napoleon during battle (1805)?" },
  proposition: { text: "Austerlitz" },
  about_event: {
    canonical_event_id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
    title: "Battle of Austerlitz",
    event_type: "battle",
    time_surface: "1805",
  },
};

describe("modelDebate", () => {
  it("classifies a battle debate without locatedIn or coerced January dates", () => {
    const graph = modelDebate(fact);
    expect(graph.category).toBe("battle");
    const event = graph.atoms.find((a) => a.role === "event");
    expect(event?.name).toBe(
      "canonical-event:aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
    );
    expect(event?.sameAs).toBe(
      "talaria://canonical-event/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
    );
    expect(event?.startDate).toBeUndefined();
    expect(graph.triples.map((t) => t.role).sort()).toEqual([
      "proposition_about_event",
      "question_has_category",
      "question_has_proposition",
    ]);
    expect(graph.triples.some((t) => t.role.includes("located"))).toBe(false);
    expect(graph.voteTripleRole).toBe("question_has_proposition");
  });

  it("keeps role structure stable across calls", () => {
    const a = modelDebate(fact);
    const b = modelDebate(fact);
    expect(a.voteTripleRole).toBe(b.voteTripleRole);
    expect(a.atoms.map((x) => x.role)).toEqual(b.atoms.map((x) => x.role));
  });
});

describe("startDateField", () => {
  it("accepts only full calendar days", () => {
    expect(startDateField("1805")).toBeUndefined();
    expect(startDateField("1805-12")).toBeUndefined();
    expect(startDateField("1805-12-02")).toBe("1805-12-02");
  });
});

describe("atomDataFromPinUri", () => {
  it("keeps the ipfs uri as atom data", () => {
    const uri =
      "ipfs://bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
    expect(atomDataFromPinUri(uri)).toBe(uri);
    expect(() => atomDataFromPinUri("https://example.com")).toThrow(/ipfs/);
  });
});
