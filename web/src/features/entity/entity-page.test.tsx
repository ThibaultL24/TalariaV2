import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { useExplorerStore } from "@/stores/explorer-store";
import { EntityPage, databaseTabs } from "./entity-page";

vi.mock("./entity-map", () => ({ EntityMap: () => <div>Map surface</div> }));
vi.mock("@/components/timeline/historical-timeline", () => ({
  HistoricalTimeline: () => <div>Canvas surface</div>,
}));
vi.mock("@/components/detail/event-detail-card", () => ({
  EventDetailCard: ({ event }: { event: { title: string } }) => <div>{event.title}</div>,
}));
const event = {
  id: "event-1",
  entity_id: "person-1",
  title: "A sourced occurrence",
  event_type: "travel",
  time: { kind: "approx", start: "1855", precision: "year" },
  map_eligible: true,
};
beforeEach(() => {
  useExplorerStore.getState().clearEntity();
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: string) => {
      const value = input.includes("/overview?")
        ? {
            entity: { id: "person-1", label: "Victor Hugo" },
            stats: { timeline_events: 2, mapped_events: 1, places: 1 },
            time_bounds: { from: 1802, to: 1885 },
          }
        : input.includes("/events/event-1")
          ? { event }
          : { events: [event], count: 1, pagination: { next_cursor: null } };
      return { ok: true, json: async () => value };
    }),
  );
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
function open(url: string) {
  render(
    <MemoryRouter initialEntries={[url]}>
      <Routes>
        <Route path="/entities/:entityId/:view" element={<EntityPage />} />
        <Route path="/explorer" element={<EntityPage />} />
      </Routes>
    </MemoryRouter>,
  );
}
test("event deep link opens evidence detail with Intuition disabled", async () => {
  open("/entities/person-1/timeline?event=event-1");
  await screen.findByRole("heading", { name: "Victor Hugo" });
  await screen.findByText("A sourced occurrence");
  expect(
    screen.getByRole("link", { name: "View on map" }).getAttribute("href"),
  ).toContain("event=event-1");
  expect(screen.queryByRole("list")).toBeNull();
  expect(vi.mocked(fetch).mock.calls.every(([url]) => !String(url).includes("intuition"))).toBe(true);
});
test("timeline view uses the histography canvas without numeric filter fields", async () => {
  open("/entities/person-1/timeline");
  await screen.findByRole("heading", { name: "Victor Hugo" });
  expect(await screen.findByText("Canvas surface")).toBeTruthy();
  expect(screen.queryByLabelText("From")).toBeNull();
  expect(vi.mocked(fetch).mock.calls.filter(([url]) => String(url).includes("timeline?")).every(([url]) => !String(url).includes("from="))).toBe(true);
});
test("Explorer opens an overview search without requesting an empty entity", async () => {
  open("/explorer");
  expect(screen.getByRole("heading", { name: "Overview" })).toBeTruthy();
  expect(screen.getByRole("searchbox")).toBeTruthy();
  expect(screen.queryByText("Map surface")).toBeNull();
  expect(vi.mocked(fetch).mock.calls).toHaveLength(0);
});
test("database tabs include empty databases and use the canonical OpenAlex key", () => {
  const tabs = databaseTabs([{name: "open_alex", count: 25}]);
  expect(tabs.find(p => p.name === "open_alex")?.count).toBe(25);
  expect(tabs.find(p => p.name === "wikipedia")?.count).toBe(0);
  expect(tabs.filter(p => p.name === "open_alex")).toHaveLength(1);
});

test("database tabs filter through the API and keep complete provider counts", async () => {
  vi.mocked(fetch).mockImplementation(async (input) => ({ ok: true, json: async () => String(input).includes("bibliography") ? {
    items: [], providers: [{ name: "hal", count: 201 }, { name: "gallica", count: 12 }], next_cursor: null,
  } : { entity: { id: "person-1", label: "Victor Hugo" }, stats: {}, time_bounds: { from: 1802, to: 1885 } } }) as Response);
  open("/entities/person-1/sources");
  fireEvent.click(await screen.findByRole("button", { name: "HAL 201" }));
  await vi.waitFor(() => expect(vi.mocked(fetch).mock.calls.some(([url]) => String(url).includes("providers=hal"))).toBe(true));
  expect(screen.getByRole("button", { name: "Gallica 12" })).toBeTruthy();
});
