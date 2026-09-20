// sidecar/intuition/predicates.test.ts
import { describe, expect, it } from "vitest";
import { predicateAtomData, predicateRegistryId } from "./predicates.ts";

describe("predicates", () => {
  it("dedupes Talaria predicate atom data across calls", () => {
    expect(predicateAtomData("hasProposition")).toBe(
      predicateAtomData("hasProposition"),
    );
    expect(predicateAtomData("about")).toBe(predicateAtomData("about"));
  });

  it("keeps registry ids stable", () => {
    expect(predicateRegistryId("hasCategory")).toMatch(/^0x[0-9a-fA-F]{64}$/);
    expect(predicateRegistryId("hasProposition")).toBe(
      predicateRegistryId("hasProposition"),
    );
  });
});
