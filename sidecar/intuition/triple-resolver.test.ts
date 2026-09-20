// sidecar/intuition/triple-resolver.test.ts
import { describe, expect, it } from "vitest";
import { disputeCounterTripleId, voteTripleId } from "./triple-resolver.ts";
import { termIdFromPinUri } from "./atom-resolver.ts";

describe("triple-resolver", () => {
  it("derives a distinct counter-triple for dispute", () => {
    const vote = voteTripleId(
      ("0x" + "11".repeat(32)) as `0x${string}`,
      ("0x" + "22".repeat(32)) as `0x${string}`,
      ("0x" + "33".repeat(32)) as `0x${string}`,
    );
    const counter = disputeCounterTripleId(vote);
    expect(counter).toMatch(/^0x[0-9a-fA-F]{64}$/);
    expect(counter).not.toBe(vote);
  });
});

describe("atom-resolver", () => {
  it("computes atom id from the pinned ipfs uri", () => {
    const uri =
      "ipfs://bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
    const id = termIdFromPinUri(uri);
    expect(id).toMatch(/^0x[0-9a-fA-F]{64}$/);
    expect(termIdFromPinUri(uri)).toBe(id);
  });
});
