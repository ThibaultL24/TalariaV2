// sidecar/intuition/stance-deposit.test.ts
import { describe, expect, it } from "vitest";
import { computeMinShares } from "./stance-deposit.ts";

describe("computeMinShares", () => {
  it("applies explicit slippage and never returns zero for healthy previews", () => {
    expect(computeMinShares(10_000n, 100n)).toBe(9_900n);
    expect(computeMinShares(100n, 100n)).toBe(99n);
  });

  it("rejects zero preview shares", () => {
    expect(() => computeMinShares(0n, 100n)).toThrow(/positive/i);
  });

  it("rejects slippage that would collapse minShares", () => {
    expect(() => computeMinShares(1n, 10_000n)).toThrow(/slippage/i);
    expect(() => computeMinShares(10n, 10_000n)).toThrow(/slippage/i);
  });
});
