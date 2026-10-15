// web/src/styles/ambient.test.ts
import { readFileSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";
import { describe, expect, it } from "vitest";

const ambientPath = join(dirname(fileURLToPath(import.meta.url)), "ambient.css");

describe("ambient.css reduced motion", () => {
  it("disables decorative drift when prefers-reduced-motion", () => {
    const css = readFileSync(ambientPath, "utf8");
    expect(css).toContain("@media (prefers-reduced-motion: reduce)");
    expect(css).toMatch(/\.talaria-ambient__layer[\s\S]*animation:\s*none/);
  });
});
