// web/src/components/brand/licensed-hero-video.test.tsx
import { render } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { LicensedHeroVideo, licensedHeroVideoSource } from "./licensed-hero-video";

afterEach(() => {
  vi.unstubAllEnvs();
});

test("the documentary video stays off unless a local licensed file is enabled", () => {
  vi.stubEnv("VITE_TALARIA_LICENSED_HERO_VIDEO", "false");
  vi.stubEnv("VITE_TALARIA_LICENSED_HERO_VIDEO_SRC", "/media/licensed-hero.mp4");
  expect(licensedHeroVideoSource()).toBeUndefined();
  const view = render(<LicensedHeroVideo />);
  expect(view.container.querySelector("video")).toBeNull();
});

test("a remote preview is refused even when the flag is on", () => {
  vi.stubEnv("VITE_TALARIA_LICENSED_HERO_VIDEO", "true");
  vi.stubEnv("VITE_TALARIA_LICENSED_HERO_VIDEO_SRC", "https://example.invalid/preview.mp4");
  expect(licensedHeroVideoSource()).toBeUndefined();
});

test("reduced motion does not start playback", () => {
  vi.stubEnv("VITE_TALARIA_LICENSED_HERO_VIDEO", "true");
  vi.stubEnv("VITE_TALARIA_LICENSED_HERO_VIDEO_SRC", "/media/licensed-hero.mp4");
  const play = vi.fn(() => Promise.resolve());
  vi.spyOn(window.HTMLMediaElement.prototype, "play").mockImplementation(play);
  window.matchMedia = ((query: string) => ({
    matches: query.includes("reduce"),
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    addListener: () => undefined,
    removeListener: () => undefined,
    dispatchEvent: () => false,
    onchange: null,
  })) as typeof window.matchMedia;

  const view = render(<LicensedHeroVideo />);
  const video = view.container.querySelector("video");
  expect(video).toBeTruthy();
  expect(video?.hasAttribute("autoplay")).toBe(false);
  expect(video?.muted).toBe(true);
  expect(video?.getAttribute("poster")).toBe("/brand/hero-aegean-day.jpg");
  expect(play).not.toHaveBeenCalled();
});
