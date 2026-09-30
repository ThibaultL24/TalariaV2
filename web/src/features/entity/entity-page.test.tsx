import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { EntityPage } from "./entity-page";

vi.mock("./entity-map", () => ({ EntityMap: () => <div>Map surface</div> }));
vi.mock("./timeline-canvas", () => ({
  TimelineCanvas: () => <div>Canvas surface</div>,
}));
vi.mock("@/components/detail/event-detail-card", () => ({
  EventDetailCard: ({
    event,
    showIntuition,
  }: {
    event: { title: string };
    showIntuition: boolean;
  }) => (
    <div>
      {event.title}
      <span>{String(showIntuition)}</span>
    </div>
  ),
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
      </Routes>
    </MemoryRouter>,
  );
}
test("event deep link opens evidence detail with Intuition disabled", async () => {
  open("/entities/person-1/timeline?event=event-1");
  await screen.findByRole("heading", { name: "Victor Hugo" });
  await screen.findByText("false");
  expect(
    screen.getByRole("link", { name: "View on map" }).getAttribute("href"),
  ).toContain("event=event-1");
  expect(screen.getByText("≈ 1855")).toBeTruthy();
});
test("time filter is sent to the entity-scoped API", async () => {
  open("/entities/person-1/timeline");
  await screen.findByRole("heading", { name: "Victor Hugo" });
  fireEvent.change(screen.getByLabelText("From"), {
    target: { value: "1851" },
  });
  await vi.waitFor(() =>
    expect(
      vi
        .mocked(fetch)
        .mock.calls.some(
          ([url]) =>
            String(url).includes("timeline?") &&
            String(url).includes("from=1851"),
        ),
    ).toBe(true),
  );
});
