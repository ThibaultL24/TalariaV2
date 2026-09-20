// sidecar/intuition/model.logical.test.ts
import { describe, expect, it } from "vitest";
import { modelDebate } from "./model.ts";
import { SCHEMA_VERSION_V2 } from "./schemas.ts";

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

describe("modelDebate logical graph", () => {
  it("links roles instead of provisional chain term ids", () => {
    const graph = modelDebate(fact);
    expect(graph.triples).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          role: "question_has_proposition",
          subjectRole: "question",
          predicateKey: "hasProposition",
          objectRole: "proposition",
        }),
        expect.objectContaining({
          role: "proposition_about_event",
          subjectRole: "proposition",
          predicateKey: "about",
          objectRole: "event",
        }),
      ]),
    );
    for (const triple of graph.triples) {
      expect(triple).not.toHaveProperty("subjectId");
      expect(triple).not.toHaveProperty("objectId");
    }
  });

  it("does not attach a definitive atomId before pin", () => {
    const graph = modelDebate(fact);
    for (const atom of graph.atoms) {
      expect(atom).not.toHaveProperty("atomId");
      expect(atom.pin).toMatchObject({
        name: expect.any(String),
        description: expect.any(String),
      });
    }
  });

  it("never coerces year-only time into a full startDate", () => {
    const graph = modelDebate(fact);
    const event = graph.atoms.find((a) => a.role === "event");
    expect(event?.startDate).toBeUndefined();
  });

  it("keeps logical structure stable across calls", () => {
    const a = modelDebate(fact);
    const b = modelDebate(fact);
    expect(a.atoms.map((x) => x.role)).toEqual(b.atoms.map((x) => x.role));
    expect(a.triples.map((x) => x.role)).toEqual(b.triples.map((x) => x.role));
    expect(a.voteTripleRole).toBe("question_has_proposition");
  });
});
