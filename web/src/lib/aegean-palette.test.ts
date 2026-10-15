// web/src/lib/aegean-palette.test.ts
import { describe, expect, it } from "vitest";
import { aegeanPixiColors } from "./aegean-palette";

describe("aegeanPixiColors", () => {
  it("maps Aegean canvas and selection rings for Pixi", () => {
    expect(aegeanPixiColors("light").background).toBe(0xf7f3ea);
    expect(aegeanPixiColors("dark").background).toBe(0x08151c);
    expect(aegeanPixiColors("light").selectedRing).toBe(0xe76f3c);
    expect(aegeanPixiColors("dark").selectedRing).toBe(0xf07a45);
  });
});
