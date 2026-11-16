// web/src/features/istoria/biography-filters.test.ts
import { expect, test } from "vitest";
import { biographyFilters } from "./biography-filters";

test("a chapter stays on the timeline while the frieze owns the year window", () => {
  const params = new URLSearchParams("group=exile&from=1855&to=1870&types=residence");
  expect(biographyFilters(params, "timeline")).toBe("types=residence&group=exile");
  expect(biographyFilters(params, "map")).toBe("from=1855&to=1870&types=residence&group=exile");
});

test("a biography without a chapter keeps the previous filters", () => {
  const params = new URLSearchParams("types=battle");
  expect(biographyFilters(params, "timeline")).toBe("types=battle");
  expect(biographyFilters(params, "overview")).toBe("types=battle");
});
