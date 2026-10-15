// web/src/lib/agora-layers.test.ts
import { describe, expect, it } from "vitest";
import { isStanceClaimKind } from "@/lib/agora-taxonomy";

describe("Agora layer separation", () => {
  it("does not treat comments, arguments, or stance as the same kind", () => {
    expect("comment").not.toBe("argument");
    expect("argument").not.toBe("support");
    expect("support").not.toBe("intuition_signal");
    expect("user").not.toBe("pipeline");
    expect("source").not.toBe("comment_attachment");
  });

  it("keeps Intuition deposits on stance claims only", () => {
    expect(isStanceClaimKind("theory")).toBe(true);
    expect(isStanceClaimKind("controversy")).toBe(true);
    expect(isStanceClaimKind("debate_stance")).toBe(true);
    expect(isStanceClaimKind("anecdote")).toBe(false);
  });
});
