// web/src/features/istoria/identity-offer.test.tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { IdentityOffer } from "./identity-offer";

describe("IdentityOffer", () => {
  test("an unknown QID can be researched and a bare name cannot", () => {
    const onResearch = vi.fn();
    const { rerender } = render(
      <IdentityOffer
        identity={{ label: "Aristotle", qid: "Q868", known_locally: false }}
        locale="en"
        busy={false}
        onResearch={onResearch}
      />,
    );
    expect(screen.getByRole("button", { name: "Start research" })).toBeTruthy();
    rerender(
      <IdentityOffer
        identity={{ label: "Someone", qid: null, known_locally: false }}
        locale="fr"
        busy={false}
        onResearch={onResearch}
      />,
    );
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.getByText(/Aucun identifiant Wikidata/)).toBeTruthy();
  });

  test("a failed research is not shown as finished work", () => {
    render(
      <IdentityOffer
        identity={{ label: "Aristotle", qid: "Q868", known_locally: false }}
        locale="en"
        busy={false}
        error="failed"
        onResearch={() => undefined}
      />,
    );
    expect(screen.getByRole("alert").textContent).toBe("failed");
    expect(screen.queryByRole("status")).toBeNull();
  });
});
