import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TimelineCanvas, layoutDots } from "./timeline-canvas";
import type { TimelineEvent } from "@/lib/api";
const event = (id: string, start: string, kind = "exact") => ({ id, title: `Moment ${id}`, event_type: "travel", time: { kind, start, precision: "year" } }) as TimelineEvent;
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });
test("dense same-year events stay individually reachable without inventing dates", () => {
  const dots = layoutDots(Array.from({ length: 100 }, (_, i) => event(String(i), "1855")), 1800, 1900, 800);
  expect(new Set(dots.map(p => p.x)).size).toBe(1);
  expect(new Set(dots.map(p => p.row)).size).toBe(100);
  expect(layoutDots([event("bce", "-0044")], -100, 0, 800)).toHaveLength(1);
  expect(layoutDots([event("unknown", "1855", "unknown")], 1800, 1900, 800)).toHaveLength(0);
});
test("points open their event, show typed precision and support zoom", () => {
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
  const select = vi.fn(), zoom = vi.fn();
  render(<TimelineCanvas events={[event("a", "1855", "approx"), event("b", "", "unknown")]} from={1800} to={1900} onSelect={select} onZoom={zoom} />);
  const dot = screen.getByRole("button", { name: "≈ 1855 · Moment a" });
  fireEvent.focus(dot);
  fireEvent.click(dot);
  expect(select).toHaveBeenCalledWith("a");
  fireEvent.click(screen.getByRole("button", { name: "Moment b" }));
  expect(select).toHaveBeenCalledWith("b");
  fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
  expect(zoom).toHaveBeenCalledWith(1825, 1875);
  expect(screen.queryByRole("list")).toBeNull();
});
