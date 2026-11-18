import { describe, expect, it } from "vitest";
import { rankPersonSuggestions, researchDecision } from "./search-suggestions";
import type { SearchSuggestion } from "@/lib/schemas/entity";

function person(qid: string, label: string, extra: Partial<SearchSuggestion> = {}): SearchSuggestion {
  return { qid, label, known_locally: false, event_count: 0, ...extra };
}

describe("rankPersonSuggestions", () => {
  it("keeps Dumas father and son with distinct QIDs", () => {
    const out = rankPersonSuggestions("Alexandre Dumas", [
      person("Q187608", "Alexandre Dumas", { known_locally: true, entity_id: "father", event_count: 500 }),
      person("Q193129", "Alexandre Dumas", { known_locally: true, entity_id: "son", event_count: 15 }),
      person("Q187608", "A. Dumas"),
    ]);
    expect(out.map((v) => v.qid)).toEqual(["Q187608", "Q193129"]);
  });
  it("prioritizes exact matches and never returns more than five", () => {
    const out = rankPersonSuggestions("Victor Hugo", [
      person("Q1", "Victor Hugo Museum", { event_count: 900 }),
      person("Q2", "Victor Hugo"),
      ...Array.from({ length: 9 }, (_, i) => person(`Q${i+3}`, `Victor Hugo ${i}`)),
    ]);
    expect(out).toHaveLength(5);
    expect(out[0].qid).toBe("Q2");
  });
  it("keeps people ahead of a place that copies the searched name", () => {
    const out = rankPersonSuggestions("Dumas", [
      person("Qtown", "Dumas", { description: "human settlement in Mississippi" }),
      person("Qson", "Alexandre Dumas fils", { description: "French writer and dramatist" }),
      person("Qfather", "Alexandre Dumas", { description: "French writer and dramatist" }),
    ]);
    expect(out.map((item) => item.qid)).toEqual(["Qfather", "Qson", "Qtown"]);
  });
  it("drops an event phrase that is not a person", () => {
    const out = rankPersonSuggestions("Napoleon", [
      person("Q517", "Napoleon", { known_locally: true, entity_id: "napoleon", description: "French emperor" }),
      person("", "Napoleon's arduous retreat from Moscow", { qid: null }),
      person("", "Napoleon's army", { qid: null }),
      person("", "Napoleon's plan of attack on Ligny", { qid: null }),
      person("", "avenue Napoléon-Bonaparte", { qid: null, known_locally: true, description: "avenue Napoléon-Bonaparte" }),
      person("", "Napoléon", { qid: null, known_locally: true, description: "Napoléon" }),
    ]);
    expect(out.map((item) => item.label)).toEqual(["Napoleon"]);
  });
  it("does not merge similar but distinct people", () => {
    expect(rankPersonSuggestions("Dumas", [
      person("Q1", "Dumas père"), person("Q2", "Dumas fils"),
    ])).toHaveLength(2);
  });
  it("opens a known person, offers a selected QID, and refuses a name without one", () => {
    expect(researchDecision(person("Q517", "Napoleon", { known_locally: true, entity_id: "n" }))).toBe("open");
    expect(researchDecision(person("Q868", "Aristotle"))).toBe("offer");
    const unnamed = rankPersonSuggestions("Marceline", [])[0];
    expect(unnamed.qid).toBeNull();
    expect(researchDecision(unnamed)).toBe("unresolved");
  });
  it("keeps manual fallback and ignores development noise", () => {
    expect(rankPersonSuggestions("Unknown Historian", [])[0].label).toBe("Unknown Historian");
    expect(rankPersonSuggestions("Napoleon", [
      person("Q0", "Napoleon LotD 11111111-2222-3333-4444-555555555555"),
      person("Q517", "Napoleon"),
    ])).toHaveLength(1);
  });
});
