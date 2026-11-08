// web/src/features/poreia/poreia-search.test.ts
import { expect, test } from "vitest";
import { formatPoreiaDay, poreiaSearchSuffix, readPoreiaDestination } from "./poreia-search";

test("a destination search stays in the URL when a person is opened", () => {
  const params = new URLSearchParams("destination=Q90&dlat=48.8566&dlon=2.3522&radius=10&site=site-1");
  expect(poreiaSearchSuffix("poreia", params)).toBe("?destination=Q90&dlat=48.8566&dlon=2.3522&radius=10&site=site-1");
  expect(poreiaSearchSuffix("istoria", params)).toBe("");
});

test("an incomplete destination is ignored", () => {
  expect(readPoreiaDestination(new URLSearchParams("destination=Q90"))).toBeUndefined();
  expect(readPoreiaDestination(new URLSearchParams("destination=Paris&dlat=48&dlon=2&radius=10"))).toBeUndefined();
});

test("a day-precision date does not invent a clock time", () => {
  const label = formatPoreiaDay("2024-06-01", "fr");
  expect(label).not.toMatch(/\d{1,2}:\d{2}/);
  expect(label).toContain("2024");
});
