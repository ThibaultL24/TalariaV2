// web/src/hooks/use-media-query.test.ts
import { renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useMediaQuery } from "./use-media-query";

afterEach(() => {
  vi.unstubAllGlobals();
});

test("follows matchMedia", () => {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: query.includes("768"),
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
  const { result } = renderHook(() => useMediaQuery("(max-width: 768px)"));
  expect(result.current).toBe(true);
});
