// web/src/components/agora/claim-card.test.tsx
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ClaimCard } from "./claim-card";

const sessionState = vi.hoisted(() => ({
  authenticated: false,
  user: null as { id: string } | null,
}));

const fetchClaimSources = vi.fn();
const fetchClaimArguments = vi.fn();
const fetchClaimComments = vi.fn();
const fetchClaimIntuition = vi.fn();

vi.mock("@/hooks/use-talaria-session", () => ({
  useTalariaSession: () => sessionState,
}));

vi.mock("@/hooks/use-intuition-wallet", () => ({
  useIntuitionWallet: () => ({
    address: undefined,
    chainId: 13579,
    isConnected: false,
    isSupportedChain: true,
  }),
}));

vi.mock("wagmi", () => ({
  useSwitchChain: () => ({ switchChain: vi.fn() }),
}));

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return {
    ...actual,
    fetchClaimSources: (...args: unknown[]) => fetchClaimSources(...args),
    fetchClaimArguments: (...args: unknown[]) => fetchClaimArguments(...args),
    fetchClaimComments: (...args: unknown[]) => fetchClaimComments(...args),
    fetchClaimIntuition: (...args: unknown[]) => fetchClaimIntuition(...args),
  };
});

const claim = {
  id: "claim-1",
  claim_kind: "theory",
  text: "Napoleon was poisoned",
  epistemic_status: "disputed",
  relation_to_subject: "historiography",
  confidence: 0.4,
  debate_type: "controversy",
  evidence_layer: "historiography",
  evidence: [
    {
      id: "ev-1",
      source_system: "wikipedia",
      source_kind: "wikipedia",
      quote: "Corpus quote from Wikipedia",
      locator: null,
      confidence: 0.5,
    },
  ],
};

describe("ClaimCard", () => {
  beforeEach(() => {
    fetchClaimSources.mockReset();
    fetchClaimArguments.mockReset();
    fetchClaimComments.mockReset();
    fetchClaimIntuition.mockReset();
    fetchClaimSources.mockResolvedValue({ items: [] });
    fetchClaimArguments.mockResolvedValue({ items: [] });
    fetchClaimComments.mockResolvedValue({ items: [], next_cursor: null });
    fetchClaimIntuition.mockResolvedValue({ status: "not_published", claim_id: "claim-1" });
  });

  afterEach(() => cleanup());

  it("shows metadata and stance counts without loading details", () => {
    render(
      <ClaimCard
        claim={claim}
        summary={{
          target_id: "claim-1",
          counts: { support: 2, dispute: 1, uncertain: 0, save: 0 },
          comment_count: 3,
          argument_count: 4,
          source_count: 5,
          mine: [],
        }}
      />,
    );
    expect(screen.getByText("Napoleon was poisoned")).toBeTruthy();
    expect(screen.getByText(/2 Support/)).toBeTruthy();
    expect(screen.queryByText("Corpus quote from Wikipedia")).toBeNull();
    expect(fetchClaimSources).not.toHaveBeenCalled();
    expect(fetchClaimComments).not.toHaveBeenCalled();
    expect(fetchClaimIntuition).not.toHaveBeenCalled();
  });

  it("loads evidence, sources, arguments and comments on open", async () => {
    render(
      <ClaimCard
        claim={claim}
        summary={{
          target_id: "claim-1",
          counts: { support: 0, dispute: 0, uncertain: 0, save: 0 },
          comment_count: 0,
          argument_count: 0,
          source_count: 0,
          mine: [],
        }}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /Open evidence/ }));
    expect(await screen.findByText("Corpus quote from Wikipedia")).toBeTruthy();
    await waitFor(() => expect(fetchClaimSources).toHaveBeenCalled());
    await waitFor(() => expect(fetchClaimArguments).toHaveBeenCalled());
    await waitFor(() => expect(fetchClaimComments).toHaveBeenCalled());
    expect(await screen.findByText("This claim is not published on Intuition")).toBeTruthy();
    expect(screen.getByText("No sources added yet")).toBeTruthy();
    expect(screen.getByText("No structured arguments yet")).toBeTruthy();
    expect(screen.getByText("No comments yet")).toBeTruthy();
  });
});
