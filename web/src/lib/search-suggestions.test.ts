import { describe, expect, it } from "vitest";
import { rankPersonSuggestions } from "./search-suggestions";
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
  it("does not merge similar but distinct people", () => {
    expect(rankPersonSuggestions("Dumas", [
      person("Q1", "Dumas père"), person("Q2", "Dumas fils"),
    ])).toHaveLength(2);
  });
  it("keeps manual fallback and ignores development noise", () => {
    expect(rankPersonSuggestions("Unknown Historian", [])[0].label).toBe("Unknown Historian");
    expect(rankPersonSuggestions("Napoleon", [
      person("Q0", "Napoleon LotD 11111111-2222-3333-4444-555555555555"),
      person("Q517", "Napoleon"),
    ])).toHaveLength(1);
  });
});
