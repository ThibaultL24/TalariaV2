// web/src/styles/v3-mobile-stage.test.ts
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "vitest";

const css = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "v3.css"), "utf8");

test("mobile scholar stage uses 100dvh, overflow-x hidden, and iOS safe areas", () => {
  expect(css).toContain("@media (max-width: 768px)");
  expect(css).toContain(".v3-shell--mobile-stage");
  expect(css).toMatch(/\.v3-shell--mobile-stage \{[\s\S]*overflow-x:\s*hidden/);
  expect(css).toMatch(/\.v3-shell--mobile-stage \{[\s\S]*height:\s*100dvh/);
  expect(css).toContain("env(safe-area-inset-top)");
  expect(css).toContain("env(safe-area-inset-bottom)");
  expect(css).toMatch(/\.v3-shell--mobile-stage \.v3-detail \{[\s\S]*max-height:\s*min\(46dvh/);
});
