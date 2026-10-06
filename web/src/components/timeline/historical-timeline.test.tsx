// web/src/components/timeline/historical-timeline.test.tsx
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { TimelineEvent } from "@/lib/api";
import { HistoricalTimeline } from "./historical-timeline";

vi.mock("pixi.js", () => {
  class Application {
    canvas = document.createElement("canvas");
    stage = { addChild() {}, on() {}, eventMode: "", hitArea: null };
    screen = { width: 320, height: 240 };
    ticker = { add() {}, remove() {} };
    init = async () => {};
    destroy() {}
  }
  class Container {
    addChild() {}
  }
  class Graphics {
    clear() {
      return this;
    }
    circle() {
      return this;
    }
    fill() {
      return this;
    }
    stroke() {
      return this;
    }
    moveTo() {
      return this;
    }
    lineTo() {
      return this;
    }
  }
  return { Application, Container, Graphics };
});

vi.mock("@/features/entity/timeline-histography-brush", () => ({
  HistographyBrush: () => <div>brush</div>,
}));

const event: TimelineEvent = {
  id: "event-1",
  entity_id: "person-1",
  person: "Victor Hugo",
  event_type: "travel",
  epistemic_status: "sourced",
  title: "A sourced occurrence",
  time: { kind: "approx", start: "1855", precision: "year" },
  map_eligible: true,
};

beforeEach(() => {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

test("desktop timeline keeps the category list in the layout", () => {
  render(
    <HistoricalTimeline
      events={[event]}
      bounds={[1802, 1885]}
      from={1802}
      to={1885}
      mode="desktop"
      onSelect={() => {}}
      onZoom={() => {}}
    />,
  );
  expect(document.querySelector(".historical-timeline")?.getAttribute("data-mode")).toBe("desktop");
  expect(screen.queryByRole("button", { name: "Categories" })).toBeNull();
  expect(screen.getByRole("complementary", { name: "Event categories" })).toBeTruthy();
});

test("fullscreen timeline opens and closes the category drawer", () => {
  render(
    <HistoricalTimeline
      events={[event]}
      bounds={[1802, 1885]}
      from={1802}
      to={1885}
      mode="fullscreen"
      onSelect={() => {}}
      onZoom={() => {}}
    />,
  );
  expect(document.querySelector(".historical-timeline--fullscreen")).toBeTruthy();
  const drawer = document.querySelector(".historical-timeline__categories") as HTMLElement;
  expect(drawer.hidden).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Categories" }));
  expect(drawer.hidden).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Close" }));
  expect(drawer.hidden).toBe(true);
});
