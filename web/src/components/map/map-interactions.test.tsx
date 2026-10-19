import { act, render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Map, MapMouseEvent } from "maplibre-gl";
import { MapInteractions } from "./map-interactions";

describe("MapInteractions", () => {
  it("queries features only once per animation frame and avoids repeat preview renders", () => {
    const handlers = new Map<string, (e: MapMouseEvent) => void>();
    const callbacks: FrameRequestCallback[] = [];
    vi.stubGlobal("requestAnimationFrame", (fn: FrameRequestCallback) => {
      callbacks.push(fn);
      return callbacks.length;
    });
    vi.stubGlobal("cancelAnimationFrame", vi.fn());
    const canvas = { style: { cursor: "" } };
    const map = {
      on: vi.fn((name: string, cb: (e: MapMouseEvent) => void) => { handlers.set(name, cb); }),
      off: vi.fn(),
      getLayer: vi.fn(() => ({})),
      getCanvas: vi.fn(() => canvas),
      queryRenderedFeatures: vi.fn(() => [{
        id: "event-1",
        properties: { id: "event-1", title: "Battle of Austerlitz" },
      }]),
    } as unknown as Map;
    const onPreview = vi.fn();
    const { rerender, unmount } = render(
      <MapInteractions map={map} onSelectEvent={vi.fn()} onPreview={onPreview}/>,
    );
    const pointer = { point: { x: 10, y: 20 } } as MapMouseEvent;
    act(() => {
      handlers.get("mousemove")!(pointer);
      handlers.get("mousemove")!(pointer);
      callbacks.shift()!(0);
    });
    expect(map.queryRenderedFeatures).toHaveBeenCalledTimes(1);
    expect(onPreview).toHaveBeenCalledTimes(1);
    rerender(<MapInteractions map={map} onSelectEvent={vi.fn()} onPreview={onPreview}/>);
    act(() => {
      handlers.get("mousemove")!(pointer);
      callbacks.shift()!(0);
    });
    expect(map.queryRenderedFeatures).toHaveBeenCalledTimes(2);
    expect(onPreview).toHaveBeenCalledTimes(1);
    unmount();
    vi.unstubAllGlobals();
  });
});
