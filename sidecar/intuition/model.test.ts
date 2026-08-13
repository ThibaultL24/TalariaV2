// sidecar/intuition/model.test.ts
import { describe, expect, it } from "vitest";
import { modelDebate } from "./model.ts";

const fact = {
  version: "talaria.intuition_canon.v2",
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
    expect(graph.eventAtom?.name).toBe(
      "canonical-event:aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
    );
    expect(graph.eventAtom?.sameAs).toBe(
      "talaria://canonical-event/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
    );
    expect(graph.eventAtom?.startDate).toBeUndefined();
    expect(graph.triples.map((t) => t.role).sort()).toEqual([
      "proposition_about_event",
      "question_has_category",
      "question_has_proposition",
    ]);
    expect(graph.triples.some((t) => t.role.includes("located"))).toBe(false);
    expect(graph.voteTripleId.startsWith("0x")).toBe(true);
    expect(graph.voteTripleId).toHaveLength(66);
  });

  it("keeps IDs stable across calls", () => {
    const a = modelDebate(fact);
    const b = modelDebate(fact);
    expect(a.voteTripleId).toBe(b.voteTripleId);
    expect(a.eventAtom?.atomId).toBe(b.eventAtom?.atomId);
  });
});
