import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { useLocaleStore } from "@/stores/locale-store";
import { ExperienceDemoFigures } from "./experience-demo-figures";

beforeEach(() => {
  useLocaleStore.getState().setLocale("fr");
  vi.stubGlobal("fetch", vi.fn(async () => ({
    ok: true,
    json: async () => ({
      count: 2,
      items: [
        { qid: "Q517", entity_id: "person-napoleon", known_locally: true, event_count: 50, map_pin_count: 22, claim_count: 0, intuition_count: 0 },
        { qid: "Q535", entity_id: null, known_locally: false, event_count: 0, map_pin_count: 0, claim_count: 0, intuition_count: 0 },
      ],
    }),
  })));
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

test("Poreia entry presents only available demo figures and opens their heritage", async () => {
  render(<MemoryRouter><ExperienceDemoFigures experience="poreia" /></MemoryRouter>);
  const link = await screen.findByRole("link", { name: /Napoléon Bonaparte/ });
  expect(link.getAttribute("href")).toBe("/poreia/person-napoleon/heritage");
  expect(screen.queryByText("Victor Hugo")).toBeNull();
});

test("Istoria entry opens the same subject in the historical overview", async () => {
  render(<MemoryRouter><ExperienceDemoFigures experience="istoria" /></MemoryRouter>);
  const link = await screen.findByRole("link", { name: /Napoléon Bonaparte/ });
  expect(link.getAttribute("href")).toBe("/istoria/person-napoleon/overview");
});
