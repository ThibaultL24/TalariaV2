// web/src/components/timeline/historical-timeline.test.tsx
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

const pixiLifecycle = vi.hoisted(() => ({ init: vi.fn(), destroy: vi.fn() }));
import type { TimelineEvent } from "@/lib/api";
import { HistoricalTimeline } from "./historical-timeline";

vi.mock("pixi.js", () => {
  class Application {
    canvas = document.createElement("canvas");
    stage = { addChild() {}, on() {}, eventMode: "", hitArea: null };
    screen = { width: 320, height: 240 };
    ticker = { add() {}, remove() {} };
    init = async () => {
      await Promise.resolve();
      pixiLifecycle.init();
    };
    destroy() {
      pixiLifecycle.destroy();
    }
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
  pixiLifecycle.init.mockClear();
  pixiLifecycle.destroy.mockClear();
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

test("opening third publishes the narrowed window", async () => {
  const onZoom = vi.fn();
  render(
    <HistoricalTimeline
      events={[event]}
      bounds={[1800, 1830]}
      from={1800}
      to={1830}
      mode="desktop"
      onSelect={() => {}}
      onZoom={onZoom}
    />,
  );
  await Promise.resolve();
  await Promise.resolve();
  fireEvent.click(screen.getByRole("button", { name: "Opening third" }));
  expect(onZoom).toHaveBeenCalledWith(1800, 1810);
});

test("a category click still lays out after StrictMode remounts Pixi", async () => {
  const battle: TimelineEvent = { ...event, id: "battle-1", event_type: "battle", title: "A battle" };
  render(
    <StrictMode>
      <HistoricalTimeline
        events={[event, battle]}
        bounds={[1802, 1885]}
        from={1802}
        to={1885}
        mode="desktop"
        onSelect={() => {}}
        onZoom={() => {}}
      />
    </StrictMode>,
  );
  await Promise.resolve();
  await Promise.resolve();
  const host = document.querySelector(".historical-timeline__canvas-host") as HTMLElement;
  const before = host.dataset.placed;
  fireEvent.click(screen.getByRole("button", { name: /Battles/ }));
  expect(host.dataset.placed).not.toBe(before);
  expect(screen.getByRole("button", { name: /Battles/ }).className).toContain("is-active");
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

test("incremental data and changing callbacks reuse one Pixi application", async () => {
  const firstSelect = vi.fn();
  const secondSelect = vi.fn();
  const props = {
    events: [event],
    bounds: [1802, 1885] as [number, number],
    from: 1802,
    to: 1885,
    onSelect: firstSelect,
    onZoom: vi.fn(),
  };
  const { rerender, unmount } = render(<HistoricalTimeline {...props}/>);
  await waitFor(() => expect(pixiLifecycle.init).toHaveBeenCalledTimes(1));
  rerender(<HistoricalTimeline {...props} events={[event, { ...event, id: "event-2" }]} onSelect={secondSelect}/>);
  expect(pixiLifecycle.init).toHaveBeenCalledTimes(1);
  expect(pixiLifecycle.destroy).not.toHaveBeenCalled();
  unmount();
  await waitFor(() => expect(pixiLifecycle.destroy).toHaveBeenCalledTimes(1));
});
