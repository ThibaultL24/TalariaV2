import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TimelineCanvas, layoutDots } from "./timeline-canvas";
import { clampWindow, windowCenteredOn } from "./timeline-window";
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
test("a life window stays inside the span and recenters on a year", () => {
  expect(clampWindow(1800, 1900, 1790, 1810)).toEqual([1800, 1820]);
  expect(windowCenteredOn(1800, 1900, 20, 1855)).toEqual([1845, 1865]);
  expect(windowCenteredOn(1800, 1900, 20, 1899)).toEqual([1880, 1900]);
});
test("points open their event and the life bar replaces zoom buttons", () => {
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
  const select = vi.fn(), zoom = vi.fn();
  render(<TimelineCanvas events={[event("a", "1855", "approx"), event("b", "", "unknown")]} from={1800} to={1900} lifeFrom={1800} lifeTo={1900} onSelect={select} onZoom={zoom} />);
  const dot = screen.getByRole("button", { name: "≈ 1855 · Moment a" });
  fireEvent.focus(dot);
  fireEvent.click(dot);
  expect(select).toHaveBeenCalledWith("a");
  fireEvent.click(screen.getByRole("button", { name: "Moment b" }));
  expect(select).toHaveBeenCalledWith("b");
  expect(screen.getByRole("button", { name: "Earlier edge" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Zoom in" })).toBeNull();
});
