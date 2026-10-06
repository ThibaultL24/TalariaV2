import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { IntuitionProviders } from "@/lib/intuition/providers";
import { useExplorerStore } from "@/stores/explorer-store";
import { EntityPage, databaseTabs } from "./entity-page";

vi.mock("@/components/timeline/historical-timeline", () => ({
  HistoricalTimeline: ({ mode = "desktop" }: { mode?: string }) => (
    <div>Canvas surface {mode}</div>
  ),
}));
vi.mock("./entity-map", () => ({
  EntityMap: ({ mode = "desktop" }: { mode?: string }) => <div>Map surface {mode}</div>,
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
    <IntuitionProviders>
      <MemoryRouter initialEntries={[url]}>
        <Routes>
          <Route path="/entities/:entityId/:view" element={<EntityPage />} />
          <Route path="/explorer" element={<EntityPage />} />
        </Routes>
      </MemoryRouter>
    </IntuitionProviders>,
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
  expect(await screen.findByText(/Canvas surface/)).toBeTruthy();
  expect(screen.queryByLabelText("From")).toBeNull();
  expect(vi.mocked(fetch).mock.calls.filter(([url]) => String(url).includes("timeline?")).every(([url]) => !String(url).includes("from="))).toBe(true);
});
test("Explorer opens an overview search without requesting an empty entity", async () => {
  open("/explorer");
  expect(screen.getByRole("heading", { name: "Overview" })).toBeTruthy();
  expect(screen.getByRole("searchbox")).toBeTruthy();
  expect(screen.queryByText("Map surface")).toBeNull();
  expect(
    vi.mocked(fetch).mock.calls.every(([url]) => String(url).includes("/api/v1/auth/me")),
  ).toBe(true);
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

function stubViewport(matches: boolean) {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

test("desktop timeline keeps the main navbar and desktop canvas mode", async () => {
  stubViewport(false);
  open("/entities/person-1/timeline");
  await screen.findByRole("heading", { name: "Victor Hugo" });
  expect(screen.getByRole("navigation", { name: "Main" })).toBeTruthy();
  expect(screen.getByText("Canvas surface desktop")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Search and filters" })).toBeNull();
});

test("mobile timeline is fullscreen without the desktop navbar", async () => {
  stubViewport(true);
  open("/entities/person-1/timeline");
  await screen.findByRole("heading", { name: "Victor Hugo" });
  expect(screen.queryByRole("navigation", { name: "Main" })).toBeNull();
  expect(screen.getByRole("link", { name: "Back" })).toBeTruthy();
  expect(screen.getByText("Canvas surface fullscreen")).toBeTruthy();
  expect(document.querySelector(".v3-shell--mobile-stage")).toBeTruthy();
});

test("mobile map is fullscreen and tools open as a drawer", async () => {
  stubViewport(true);
  open("/entities/person-1/map");
  await screen.findByRole("heading", { name: "Victor Hugo" });
  expect(screen.getByText("Map surface fullscreen")).toBeTruthy();
  expect(screen.queryByRole("combobox")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Search and filters" }));
  expect(screen.getByRole("combobox")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Close" }));
  expect(screen.queryByRole("combobox")).toBeNull();
});

test("timeline to map keeps the selected event", async () => {
  stubViewport(true);
  open("/entities/person-1/timeline?event=event-1");
  await screen.findByText("A sourced occurrence");
  expect(screen.getByRole("link", { name: "View on map" }).getAttribute("href")).toContain(
    "event=event-1",
  );
});

test("map to timeline keeps the selected event", async () => {
  stubViewport(true);
  open("/entities/person-1/map?event=event-1");
  await screen.findByText("A sourced occurrence");
  expect(screen.getByRole("link", { name: "View in timeline" }).getAttribute("href")).toContain(
    "event=event-1",
  );
});

test("mobile overview is unchanged and does not enter the immersive stage", async () => {
  stubViewport(true);
  open("/entities/person-1/overview");
  await screen.findByRole("heading", { name: "Victor Hugo" });
  expect(screen.getByRole("navigation", { name: "Main" })).toBeTruthy();
  expect(document.querySelector(".v3-shell--mobile-stage")).toBeNull();
  expect(screen.queryByRole("link", { name: "Back" })).toBeNull();
});

test("mobile timeline shell marks overflow-x hidden fullscreen stage", async () => {
  stubViewport(true);
  open("/entities/person-1/timeline");
  await screen.findByText("Canvas surface fullscreen");
  expect(document.querySelector(".v3-shell--mobile-stage")).toBeTruthy();
});
