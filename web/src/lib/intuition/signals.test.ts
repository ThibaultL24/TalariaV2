// web/src/lib/intuition/signals.test.ts
import { describe, expect, it } from "vitest";
import { computeMinShares, parseTrustAmount, DEFAULT_SIGNAL_SLIPPAGE_BPS } from "./signals";

describe("claim signal math", () => {
  it("applies 1% slippage and never returns 0", () => {
    expect(DEFAULT_SIGNAL_SLIPPAGE_BPS).toBe(100n);
    expect(computeMinShares(10_000n)).toBe(9900n);
  });

  it("rejects minShares collapse", () => {
    expect(() => computeMinShares(1n)).toThrow(/collapsed/i);
  });

  it("parses TRUST amounts as 18-decimal ether", () => {
    expect(parseTrustAmount("1")).toBe(1_000_000_000_000_000_000n);
    expect(parseTrustAmount("0.5")).toBe(500_000_000_000_000_000n);
    expect(() => parseTrustAmount("0")).toThrow();
  });
});
