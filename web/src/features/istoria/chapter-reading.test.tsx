// web/src/features/istoria/chapter-reading.test.tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { expect, test } from "vitest";
import { groupChapterEvents, type ChapterEvent } from "@/lib/entity-views";
import { ChapterReading } from "./chapter-reading";

function event(partial: Partial<ChapterEvent> & Pick<ChapterEvent, "id" | "editorial_role">): ChapterEvent {
  return {
    logical_event_id: partial.id,
    display_order: 0,
    event_type: "publication",
    epistemic_status: "attested",
    title: partial.id,
    time: { kind: "unknown" },
    map_eligible: false,
    evidence_count: 0,
    ...partial,
  };
}

test("roles stay distinct and an unlocated event remains readable", () => {
  const events = [
    event({ id: "battle", editorial_role: "primary", event_type: "battle", title: "Lodi" }),
    event({ id: "book", editorial_role: "supporting", title: "A book without a place" }),
    event({ id: "context", editorial_role: "context", title: "A useful context" }),
  ];
  const grouped = groupChapterEvents(events);
  expect(grouped.primary.map((item) => item.id)).toEqual(["battle"]);
  expect(grouped.supporting.map((item) => item.id)).toEqual(["book"]);
  expect(grouped.context.map((item) => item.id)).toEqual(["context"]);
});

test("a chapter without places does not offer a map, and states that coverage is not a percentage", () => {
  render(
    <MemoryRouter>
      <ChapterReading
        group={{
          id: "g",
          slug: "exile",
          title: "Years of exile",
          group_kind: "narrative",
          summary: "An editorial introduction.",
          summary_origin: "editor",
          display_order: 0,
        }}
        chapterIndex={1}
        chapterCount={1}
        events={[
          event({ id: "book", editorial_role: "primary", title: "A book" }),
          event({ id: "note", editorial_role: "supporting", title: "A short anecdote" }),
          event({ id: "background", editorial_role: "context", title: "Useful background" }),
        ]}
        complete
        coverage={{
          members: 1,
          established: 0,
          attested: 1,
          in_review: 0,
          without_coordinates: 1,
          time_not_exact: 1,
          with_evidence: 0,
          evidence_rows: 2,
          independent_sources: 1,
          scope_defined: false,
          completeness_rate: null,
        }}
        timelineTo="/istoria/person/timeline?group=exile"
        mapTo="/istoria/person/map?group=exile"
        onSelect={() => {}}
      />
    </MemoryRouter>,
  );
  expect(screen.getByRole("heading", { name: "Years of exile" })).toBeTruthy();
  expect(screen.getByText("An editorial introduction.")).toBeTruthy();
  expect(screen.getByText(/Provenance|provenance/i)).toBeTruthy();
  expect(screen.queryByRole("link", { name: /places|lieux/i })).toBeNull();
  expect(screen.getByRole("link", { name: /chronology|chronologie/i })).toBeTruthy();
  expect(screen.getByRole("heading", { name: /Essential episodes|Épisodes essentiels/ })).toBeTruthy();
  expect(screen.getByRole("heading", { name: /Anecdotes/ })).toBeTruthy();
  expect(document.querySelector(".chapter-reading-primary ol")).toBeTruthy();
  expect(document.querySelector(".chapter-reading-supporting ul")).toBeTruthy();
  expect(document.querySelector("details.chapter-reading-context")?.hasAttribute("open")).toBe(false);
  expect(document.querySelector("details.chapter-reading-context")?.textContent).toContain("Useful background");
  expect(screen.getByText(/1 distinct source document|1 document source distinct/)).toBeTruthy();
  expect(screen.getByText(/repeat the same account|reprendre le même récit/)).toBeTruthy();
  expect(screen.getByText(/2 passages/)).toBeTruthy();
  expect(screen.getByText(/completeness|complétude/i)).toBeTruthy();
  expect(screen.queryByText(/%/)).toBeNull();
  expect(screen.queryByRole("button", { name: /next events|événements suivants/ })).toBeNull();
});

test("a longer chapter asks for the next server page", () => {
  let loads = 0;
  render(
    <MemoryRouter>
      <ChapterReading
        group={{
          id: "g",
          slug: "italian-campaign",
          title: "The first Italian campaign",
          group_kind: "chronological",
          summary: null,
          summary_origin: null,
          display_order: 0,
        }}
        chapterIndex={1}
        chapterCount={1}
        events={[event({ id: "lodi", editorial_role: "primary", title: "Lodi" })]}
        complete={false}
        onLoadMore={() => { loads += 1; }}
        timelineTo="/istoria/person/timeline?group=italian-campaign"
        mapTo="/istoria/person/map?group=italian-campaign"
        onSelect={() => {}}
      />
    </MemoryRouter>,
  );
  fireEvent.click(screen.getByRole("button", { name: /next events|événements suivants/ }));
  expect(loads).toBe(1);
});

test("a year-precision fact does not display a day found only in a passage", () => {
  render(
    <MemoryRouter>
      <ChapterReading
        group={{
          id: "g",
          slug: "italian-campaign",
          title: "The first Italian campaign",
          group_kind: "chronological",
          summary: null,
          summary_origin: null,
          display_order: 0,
        }}
        chapterIndex={1}
        chapterCount={1}
        events={[event({
          id: "montenotte",
          editorial_role: "primary",
          title: "Montenotte",
          time: { kind: "exact", precision: "year", surface: "12 April 1796", start: "1796-01-01" },
        })]}
        complete
        timelineTo="/istoria/person/timeline"
        mapTo="/istoria/person/map"
        onSelect={() => {}}
      />
    </MemoryRouter>,
  );
  expect(screen.getByText(/Recorded year: 1796|Année enregistrée : 1796/)).toBeTruthy();
  expect(screen.queryByText("12 April 1796")).toBeNull();
});
