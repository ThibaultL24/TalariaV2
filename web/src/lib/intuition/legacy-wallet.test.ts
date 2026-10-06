// web/src/lib/intuition/legacy-wallet.test.ts
import { existsSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

describe("legacy wallet cleanup", () => {
  it("removes the Zustand wallet persist store", () => {
    const store = path.resolve(__dirname, "../../stores/wallet-store.ts");
    expect(existsSync(store)).toBe(false);
  });

  it("does not keep a window.ethereum connect helper", () => {
    const helper = path.resolve(__dirname, "../wallet.ts");
    expect(existsSync(helper)).toBe(false);
  });
});
