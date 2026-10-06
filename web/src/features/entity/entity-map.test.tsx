// web/src/features/entity/entity-map.test.tsx
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { EntityMap } from "./entity-map";

const resize = vi.fn();
const fakeMap = {
  resize,
  getZoom: () => 4,
  flyTo: vi.fn(),
  on: vi.fn(),
  off: vi.fn(),
  getBounds: () => ({
    getWest: () => 0,
    getEast: () => 10,
    getSouth: () => 0,
    getNorth: () => 10,
  }),
};

vi.mock("@/components/map/map-canvas", async () => {
  const React = await import("react");
  return {
    MapCanvas: ({ onReady }: { onReady?: (map: typeof fakeMap) => void }) => {
      React.useEffect(() => {
        onReady?.(fakeMap);
      }, [onReady]);
      return React.createElement("div", null, "canvas");
    },
  };
});
vi.mock("@/components/map/map-source-manager", () => ({ MapSourceManager: () => null }));
vi.mock("@/components/map/map-layers", () => ({ MapLayers: () => null }));
vi.mock("@/components/map/map-interactions", () => ({ MapInteractions: () => null }));
vi.mock("@/lib/entity-views", () => ({
  getEntityView: async () => ({ features: [], pagination: { next_cursor: null } }),
}));

afterEach(() => {
  cleanup();
  resize.mockClear();
});

test("desktop map keeps the legend in the layout column", () => {
  render(<EntityMap id="p1" filters="" onSelect={() => {}} mode="desktop" />);
  expect(document.querySelector(".v3-map-layout")?.getAttribute("data-mode")).toBe("desktop");
  expect(screen.queryByRole("button", { name: "Legend" })).toBeNull();
});

test("fullscreen map hides the legend until opened", () => {
  render(<EntityMap id="p1" filters="" onSelect={() => {}} mode="fullscreen" />);
  expect(document.querySelector(".v3-map-layout--fullscreen")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Legend" }));
  expect(screen.getByRole("button", { name: "Legend" }).getAttribute("aria-expanded")).toBe("true");
});

test("map.resize runs after fullscreen layout and legend overlay changes", async () => {
  const { rerender } = render(<EntityMap id="p1" filters="" onSelect={() => {}} mode="desktop" />);
  await vi.waitFor(() => expect(resize).toHaveBeenCalled());
  resize.mockClear();
  rerender(<EntityMap id="p1" filters="" onSelect={() => {}} mode="fullscreen" />);
  await vi.waitFor(() => expect(resize).toHaveBeenCalled());
  resize.mockClear();
  fireEvent.click(screen.getByRole("button", { name: "Legend" }));
  await vi.waitFor(() => expect(resize).toHaveBeenCalled());
});
