// web/src/features/istoria/chapter-suggestions.test.tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { ChapterSuggestions } from "./chapter-suggestions";

describe("ChapterSuggestions", () => {
  test("a sparse life has no chapter", () => {
    const { container } = render(<ChapterSuggestions suggestions={[]} locale="fr" />);
    expect(container.textContent).toBe("");
  });

  test("a suggestion is labeled as a suggestion", () => {
    render(
      <ChapterSuggestions
        locale="fr"
        suggestions={[{ slug: "works", title_fr: "Œuvres", title_en: "Works", event_count: 8, standing: "suggestion" }]}
      />,
    );
    expect(screen.getByText(/Œuvres · 8 · suggestion/)).toBeTruthy();
    expect(screen.getByText(/pas des chapitres publiés/)).toBeTruthy();
  });
});