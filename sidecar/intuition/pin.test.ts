// sidecar/intuition/pin.test.ts
import { describe, expect, it } from "vitest";
import { atomDataFromPinUri } from "./pin.ts";

describe("atomDataFromPinUri", () => {
  it("passes through ipfs URIs", () => {
    expect(atomDataFromPinUri("ipfs://QmAbc")).toBe("ipfs://QmAbc");
  });

  it("rejects non-ipfs", () => {
    expect(() => atomDataFromPinUri("https://example.com")).toThrow(/ipfs/);
  });
});
