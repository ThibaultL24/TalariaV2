// web/src/lib/intuition/clients.test.ts
import { describe, expect, it } from "vitest";
import { applyIntuitionSdkConfig } from "./clients";

describe("Intuition SDK client config", () => {
  it("refuses to configure the SDK on an unsupported chain", () => {
    expect(() => applyIntuitionSdkConfig(1)).toThrow(/unsupported/i);
  });

  it("accepts Intuition testnet and mainnet chain ids", () => {
    expect(() => applyIntuitionSdkConfig(13579)).not.toThrow();
    expect(() => applyIntuitionSdkConfig(1155)).not.toThrow();
  });
});
