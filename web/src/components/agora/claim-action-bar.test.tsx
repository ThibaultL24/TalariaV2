// web/src/components/agora/claim-action-bar.test.tsx
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ClaimActionBar } from "./claim-action-bar";

const sessionState = vi.hoisted(() => ({
  authenticated: false,
  user: null as { id: string; wallet_address: string | null } | null,
}));

const walletState = vi.hoisted(() => ({
  address: undefined as `0x${string}` | undefined,
  chainId: 13579,
  isConnected: false,
  isSupportedChain: true,
  publicClient: undefined as
    | {
        waitForTransactionReceipt: (args: { hash: string }) => Promise<{ status: string }>;
      }
    | undefined,
  walletClient: undefined as object | undefined,
}));

const postInteraction = vi.fn();
const fetchClaimIntuition = vi.fn();
const syncClaimSignal = vi.fn();
const previewClaimSignal = vi.fn();
const depositClaimSignal = vi.fn();
const switchChain = vi.fn();

vi.mock("@/hooks/use-talaria-session", () => ({
  useTalariaSession: () => sessionState,
}));

vi.mock("@/hooks/use-intuition-wallet", () => ({
  useIntuitionWallet: () => walletState,
}));

vi.mock("wagmi", () => ({
  useSwitchChain: () => ({ switchChain }),
}));

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return {
    ...actual,
    postInteraction: (...args: unknown[]) => postInteraction(...args),
    fetchClaimIntuition: (...args: unknown[]) => fetchClaimIntuition(...args),
    syncClaimSignal: (...args: unknown[]) => syncClaimSignal(...args),
  };
});

vi.mock("@/lib/intuition/signals", async () => {
  const actual = await vi.importActual<typeof import("@/lib/intuition/signals")>(
    "@/lib/intuition/signals",
  );
  return {
    ...actual,
    previewClaimSignal: (...args: unknown[]) => previewClaimSignal(...args),
    depositClaimSignal: (...args: unknown[]) => depositClaimSignal(...args),
  };
});

const summary = {
  target_id: "claim-1",
  counts: { support: 42, dispute: 18, uncertain: 11, save: 0 },
  mine: [] as string[],
};

describe("ClaimActionBar", () => {
  beforeEach(() => {
    sessionState.authenticated = false;
    sessionState.user = null;
    walletState.address = undefined;
    walletState.chainId = 13579;
    walletState.isConnected = false;
    walletState.isSupportedChain = true;
    walletState.publicClient = undefined;
    walletState.walletClient = undefined;
    postInteraction.mockReset();
    fetchClaimIntuition.mockReset();
    syncClaimSignal.mockReset();
    previewClaimSignal.mockReset();
    depositClaimSignal.mockReset();
    switchChain.mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  it("requires Talaria sign-in before support", () => {
    render(<ClaimActionBar claimId="claim-1" claimText="Thesis" summary={summary} />);
    fireEvent.click(screen.getByRole("button", { name: "Support" }));
    expect(screen.getByRole("alert").textContent).toMatch(/sign in/i);
    expect(previewClaimSignal).not.toHaveBeenCalled();
  });

  it("blocks when Wagmi wallet differs from Talaria session", () => {
    sessionState.authenticated = true;
    sessionState.user = { id: "u1", wallet_address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" };
    walletState.isConnected = true;
    walletState.address = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    render(<ClaimActionBar claimId="claim-1" claimText="Thesis" summary={summary} />);
    fireEvent.click(screen.getByRole("button", { name: "Support" }));
    expect(screen.getByRole("alert").textContent).toMatch(/wallet changed/i);
  });

  it("prompts to switch network on the wrong chain", () => {
    sessionState.authenticated = true;
    sessionState.user = { id: "u1", wallet_address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" };
    walletState.isConnected = true;
    walletState.address = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    walletState.chainId = 1;
    walletState.isSupportedChain = false;
    render(<ClaimActionBar claimId="claim-1" claimText="Thesis" summary={summary} />);
    fireEvent.click(screen.getByRole("button", { name: "Support" }));
    expect(screen.getByRole("alert").textContent).toMatch(/wrong network/i);
    fireEvent.click(screen.getByRole("button", { name: /switch network/i }));
    expect(switchChain).toHaveBeenCalled();
  });

  it("disables Intuition when the claim is not published", async () => {
    sessionState.authenticated = true;
    sessionState.user = { id: "u1", wallet_address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" };
    walletState.isConnected = true;
    walletState.address = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    walletState.walletClient = {};
    walletState.publicClient = { waitForTransactionReceipt: vi.fn() };
    fetchClaimIntuition.mockResolvedValue({ status: "not_published", claim_id: "claim-1" });
    render(<ClaimActionBar claimId="claim-1" claimText="Thesis" summary={summary} />);
    fireEvent.click(screen.getByRole("button", { name: "Support" }));
    await waitFor(() => expect(fetchClaimIntuition).toHaveBeenCalled());
    expect(screen.getByRole("alert").textContent).toMatch(/not published/i);
    expect(previewClaimSignal).not.toHaveBeenCalled();
    expect(depositClaimSignal).not.toHaveBeenCalled();
  });

  it("records uncertain without a blockchain call", async () => {
    sessionState.authenticated = true;
    sessionState.user = { id: "u1", wallet_address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" };
    postInteraction.mockResolvedValue({
      interaction: { id: "i1" },
      created: true,
    });
    render(<ClaimActionBar claimId="claim-1" claimText="Thesis" summary={summary} />);
    fireEvent.click(screen.getByRole("button", { name: "Uncertain" }));
    await waitFor(() => expect(postInteraction).toHaveBeenCalled());
    expect(postInteraction.mock.calls[0][0].action).toBe("uncertain");
    expect(depositClaimSignal).not.toHaveBeenCalled();
  });

  it("does not sync when the wallet rejects the signature", async () => {
    sessionState.authenticated = true;
    sessionState.user = { id: "u1", wallet_address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" };
    walletState.isConnected = true;
    walletState.address = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    walletState.walletClient = {};
    walletState.publicClient = { waitForTransactionReceipt: vi.fn() };
    fetchClaimIntuition.mockResolvedValue({
      status: "ready",
      claim_id: "claim-1",
      chain_id: 13579,
      triple_term_id: "0x" + "aa".repeat(32),
      counter_term_id: null,
    });
    previewClaimSignal.mockResolvedValue({
      action: "support",
      assets: 1n,
      minShares: 1n,
      vaultTermId: "0x" + "aa".repeat(32),
      curveId: 1n,
    });
    depositClaimSignal.mockRejectedValue(new Error("User rejected the request"));
    render(<ClaimActionBar claimId="claim-1" claimText="Thesis" summary={summary} />);
    fireEvent.click(screen.getByRole("button", { name: "Support" }));
    await waitFor(() => expect(screen.getByRole("button", { name: /preview deposit/i })).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: /preview deposit/i }));
    await waitFor(() => expect(previewClaimSignal).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: /sign deposit/i }));
    await waitFor(() => expect(depositClaimSignal).toHaveBeenCalled());
    expect(syncClaimSignal).not.toHaveBeenCalled();
    expect(screen.getByRole("alert").textContent).toMatch(/cancelled/i);
  });

  it("retries Talaria sync after a confirmed receipt", async () => {
    sessionState.authenticated = true;
    sessionState.user = { id: "u1", wallet_address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" };
    walletState.isConnected = true;
    walletState.address = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    walletState.walletClient = {};
    walletState.publicClient = {
      waitForTransactionReceipt: vi.fn().mockResolvedValue({ status: "success" }),
    };
    fetchClaimIntuition.mockResolvedValue({
      status: "ready",
      claim_id: "claim-1",
      chain_id: 13579,
      triple_term_id: "0x" + "aa".repeat(32),
      counter_term_id: null,
    });
    previewClaimSignal.mockResolvedValue({
      action: "support",
      assets: 1n,
      minShares: 1n,
      vaultTermId: "0x" + "aa".repeat(32),
      curveId: 1n,
    });
    depositClaimSignal.mockResolvedValue("0x" + "cc".repeat(32));
    syncClaimSignal.mockRejectedValueOnce(new Error("network")).mockResolvedValueOnce({
      interaction: { id: "i1" },
      synced: true,
    });
    render(<ClaimActionBar claimId="claim-1" claimText="Thesis" summary={summary} />);
    fireEvent.click(screen.getByRole("button", { name: "Support" }));
    await waitFor(() => expect(screen.getByRole("button", { name: /preview deposit/i })).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: /preview deposit/i }));
    await waitFor(() => expect(previewClaimSignal).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: /sign deposit/i }));
    await waitFor(() => expect(screen.getByRole("button", { name: /retry saving/i })).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: /retry saving/i }));
    await waitFor(() => expect(syncClaimSignal).toHaveBeenCalledTimes(2));
  });

  it("syncs after a confirmed support receipt", async () => {
    const onChanged = vi.fn();
    sessionState.authenticated = true;
    sessionState.user = { id: "u1", wallet_address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" };
    walletState.isConnected = true;
    walletState.address = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    walletState.walletClient = {};
    walletState.publicClient = {
      waitForTransactionReceipt: vi.fn().mockResolvedValue({ status: "success" }),
    };
    fetchClaimIntuition.mockResolvedValue({
      status: "ready",
      claim_id: "claim-1",
      chain_id: 13579,
      triple_term_id: "0x" + "aa".repeat(32),
      counter_term_id: null,
    });
    previewClaimSignal.mockResolvedValue({
      action: "support",
      assets: 1n,
      minShares: 1n,
      vaultTermId: "0x" + "aa".repeat(32),
      curveId: 1n,
    });
    depositClaimSignal.mockResolvedValue("0x" + "dd".repeat(32));
    syncClaimSignal.mockResolvedValue({ interaction: { id: "i1" }, synced: true });
    render(
      <ClaimActionBar
        claimId="claim-1"
        claimText="Thesis"
        summary={summary}
        onChanged={onChanged}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Support" }));
    await waitFor(() => expect(screen.getByRole("button", { name: /preview deposit/i })).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: /preview deposit/i }));
    await waitFor(() => expect(previewClaimSignal).toHaveBeenCalled());
    expect(previewClaimSignal.mock.calls[0][0].action).toBe("support");
    fireEvent.click(screen.getByRole("button", { name: /sign deposit/i }));
    await waitFor(() =>
      expect(syncClaimSignal).toHaveBeenCalledWith(
        "claim-1",
        "support",
        "0x" + "dd".repeat(32),
      ),
    );
    expect(onChanged).toHaveBeenCalled();
  });

  it("previews dispute against the counter term", async () => {
    sessionState.authenticated = true;
    sessionState.user = { id: "u1", wallet_address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" };
    walletState.isConnected = true;
    walletState.address = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    walletState.walletClient = {};
    walletState.publicClient = { waitForTransactionReceipt: vi.fn() };
    fetchClaimIntuition.mockResolvedValue({
      status: "ready",
      claim_id: "claim-1",
      chain_id: 13579,
      triple_term_id: "0x" + "aa".repeat(32),
      counter_term_id: null,
    });
    previewClaimSignal.mockResolvedValue({
      action: "dispute",
      assets: 1n,
      minShares: 1n,
      vaultTermId: "0x" + "bb".repeat(32),
      curveId: 1n,
    });
    render(<ClaimActionBar claimId="claim-1" claimText="Thesis" summary={summary} />);
    fireEvent.click(screen.getByRole("button", { name: "Dispute" }));
    await waitFor(() => expect(screen.getByRole("button", { name: /preview deposit/i })).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: /preview deposit/i }));
    await waitFor(() => expect(previewClaimSignal).toHaveBeenCalled());
    expect(previewClaimSignal.mock.calls[0][0].action).toBe("dispute");
    expect(depositClaimSignal).not.toHaveBeenCalled();
  });

  it("does not sync when the receipt reverts", async () => {
    sessionState.authenticated = true;
    sessionState.user = { id: "u1", wallet_address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" };
    walletState.isConnected = true;
    walletState.address = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    walletState.walletClient = {};
    walletState.publicClient = {
      waitForTransactionReceipt: vi.fn().mockResolvedValue({ status: "reverted" }),
    };
    fetchClaimIntuition.mockResolvedValue({
      status: "ready",
      claim_id: "claim-1",
      chain_id: 13579,
      triple_term_id: "0x" + "aa".repeat(32),
      counter_term_id: null,
    });
    previewClaimSignal.mockResolvedValue({
      action: "support",
      assets: 1n,
      minShares: 1n,
      vaultTermId: "0x" + "aa".repeat(32),
      curveId: 1n,
    });
    depositClaimSignal.mockResolvedValue("0x" + "ee".repeat(32));
    render(<ClaimActionBar claimId="claim-1" claimText="Thesis" summary={summary} />);
    fireEvent.click(screen.getByRole("button", { name: "Support" }));
    await waitFor(() => expect(screen.getByRole("button", { name: /preview deposit/i })).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: /preview deposit/i }));
    await waitFor(() => expect(previewClaimSignal).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: /sign deposit/i }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toMatch(/failed/i));
    expect(syncClaimSignal).not.toHaveBeenCalled();
  });
});
