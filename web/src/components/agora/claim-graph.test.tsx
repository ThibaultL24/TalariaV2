// web/src/components/agora/claim-graph.test.tsx
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ClaimGraph } from "./claim-graph";

const sessionState = vi.hoisted(() => ({
  authenticated: false,
  user: null as { id: string } | null,
}));

const fetchClaimSources = vi.fn();
const fetchClaimArguments = vi.fn();
const postClaimSource = vi.fn();
const postClaimArgument = vi.fn();

vi.mock("@/hooks/use-talaria-session", () => ({
  useTalariaSession: () => sessionState,
}));

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return {
    ...actual,
    fetchClaimSources: (...args: unknown[]) => fetchClaimSources(...args),
    fetchClaimArguments: (...args: unknown[]) => fetchClaimArguments(...args),
    postClaimSource: (...args: unknown[]) => postClaimSource(...args),
    postClaimArgument: (...args: unknown[]) => postClaimArgument(...args),
  };
});

const bibliography = [
  {
    id: "doc-1",
    title: "HAL article",
    document_type: "article",
    source_kind: "hal",
    external_id: "hal-1",
    academic_status: "unknown",
    epistemic: "bibliographic_resource",
    link: { relation: "about", score: 1 },
  },
];

describe("ClaimGraph", () => {
  beforeEach(() => {
    sessionState.authenticated = false;
    fetchClaimSources.mockReset();
    fetchClaimArguments.mockReset();
    postClaimSource.mockReset();
    postClaimArgument.mockReset();
    fetchClaimSources.mockResolvedValue({ items: [] });
    fetchClaimArguments.mockResolvedValue({ items: [] });
  });

  afterEach(() => cleanup());

  it("shows counts without fetching arguments until opened", () => {
    render(
      <ClaimGraph claimId="c1" argumentCount={2} sourceCount={1} bibliography={bibliography} />,
    );
    expect(screen.getByRole("button", { name: /2 arguments/ })).toBeTruthy();
    expect(fetchClaimArguments).not.toHaveBeenCalled();
  });

  it("asks anonymous users to sign in", async () => {
    render(
      <ClaimGraph claimId="c1" argumentCount={0} sourceCount={0} bibliography={bibliography} />,
    );
    fireEvent.click(screen.getByRole("button", { name: /0 arguments/ }));
    await waitFor(() => expect(fetchClaimSources).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: "Add source" }));
    expect(await screen.findByText(/Sign in to Talaria to add a source or argument/)).toBeTruthy();
    expect(postClaimSource).not.toHaveBeenCalled();
  });

  it("adds a source and a support argument separately from comments", async () => {
    sessionState.authenticated = true;
    postClaimSource.mockResolvedValue({
      source: {
        corpus_document_id: "doc-1",
        title: "HAL article",
        source_kind: "hal",
        document_type: "article",
        created_at: "2026-01-01T00:00:00Z",
      },
    });
    postClaimArgument.mockResolvedValue({
      argument: {
        id: "a1",
        target_claim_id: "c1",
        relation: "supports",
        statement: "Support statement",
        origin: "user",
        contribution_status: "proposed",
        created_at: "2026-01-01T00:00:00Z",
        author: { id: "u1", display_name: "Ada" },
        sources: [
          {
            corpus_document_id: "doc-1",
            title: "HAL article",
            source_kind: "hal",
            document_type: "article",
            created_at: "2026-01-01T00:00:00Z",
          },
        ],
        evidence: [{ id: "e1", quote: "quote", source_system: "hal" }],
      },
    });
    render(
      <ClaimGraph claimId="c1" argumentCount={0} sourceCount={0} bibliography={bibliography} />,
    );
    fireEvent.click(screen.getByRole("button", { name: /0 arguments/ }));
    await waitFor(() => expect(fetchClaimArguments).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: "Add source" }));
    fireEvent.click(screen.getAllByRole("button", { name: "Add source" })[1]);
    await waitFor(() => expect(postClaimSource).toHaveBeenCalledWith("c1", "doc-1"));
    fireEvent.click(screen.getByRole("button", { name: "Add argument" }));
    fireEvent.change(screen.getByLabelText("Statement"), { target: { value: "Support statement" } });
    fireEvent.change(screen.getByLabelText("Evidence excerpt"), { target: { value: "quote" } });
    fireEvent.click(screen.getByRole("button", { name: "Submit argument" }));
    await waitFor(() => expect(postClaimArgument).toHaveBeenCalled());
    expect(await screen.findByText("Support statement")).toBeTruthy();
    expect(document.querySelector(".argument-card__source")?.textContent).toContain("HAL article");
    expect(screen.getAllByText(/Community contribution/).length).toBeGreaterThanOrEqual(2);
    expect(screen.queryByText("Open comments")).toBeNull();
  });
});
