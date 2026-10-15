// web/src/features/visit/visit-page.test.tsx
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { VisitPage } from "./visit-page";

vi.mock("./visit-map", () => ({ VisitMap: () => <div>Visit map</div> }));
vi.mock("@/components/search/entity-search-box", () => ({
  EntitySearchBox: () => <div>Search</div>,
}));

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

test("visit keeps the person and shows unknown visitability", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: string) => {
      const url = String(input);
      if (url.includes("/visit/sites")) {
        return {
          ok: true,
          json: async () => ({
            items: [
              {
                id: "site-1",
                place_label: "Hauteville House",
                category: "residence",
                relation: "former_residence",
                temporal_class: "lived",
                visitability: "unknown",
                summary: "Residence",
                provenance: { kind: "canonical_event" },
              },
            ],
          }),
        };
      }
      if (url.includes("/visit/map")) return { ok: true, json: async () => ({ type: "FeatureCollection", features: [] }) };
      if (url.includes("/commemorations")) {
        return {
          ok: true,
          json: async () => ({
            items: [{ id: "c1", title: "Hommage 2005", occurrence_status: "past", place_persistence: "extant", bookable: false }],
          }),
        };
      }
      return { ok: false, json: async () => ({}) };
    }),
  );
  render(
    <MemoryRouter initialEntries={["/visit/11111111-1111-4111-8111-111111111111"]}>
      <Routes>
        <Route path="/visit/:entityKey" element={<VisitPage />} />
      </Routes>
    </MemoryRouter>,
  );
  expect((await screen.findAllByText("Hauteville House")).length).toBeGreaterThan(0);
  expect(screen.getByText(/visitability unknown/)).toBeTruthy();
  expect(screen.getByText(/not bookable/)).toBeTruthy();
  expect(screen.getByRole("link", { name: /Understand this life/ }).getAttribute("href")).toBe(
    "/entities/11111111-1111-4111-8111-111111111111/map",
  );
});
