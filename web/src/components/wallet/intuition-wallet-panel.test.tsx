// web/src/components/wallet/intuition-wallet-panel.test.tsx
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { IntuitionWalletPanel } from "./intuition-wallet-panel";

vi.mock("@/hooks/use-intuition-wallet", () => ({
  useIntuitionWallet: () => ({
    address: undefined,
    chainId: 13579,
    isConnected: false,
    isSupportedChain: true,
    walletClient: undefined,
  }),
}));

vi.mock("@/hooks/use-talaria-session", () => ({
  useTalariaSession: () => ({
    authenticated: false,
    signingIn: false,
    signIn: vi.fn(),
  }),
}));

vi.mock("wagmi", () => ({
  useConnect: () => ({ connect: vi.fn(), isPending: false, error: null }),
  useConnectors: () => [{ id: "injected", name: "Injected", type: "injected" }],
  useDisconnect: () => ({ disconnect: vi.fn(), isPending: false }),
  useSwitchChain: () => ({ switchChain: vi.fn(), isPending: false }),
  useBalance: () => ({ data: undefined, isLoading: false }),
}));

describe("IntuitionWalletPanel", () => {
  afterEach(() => cleanup());

  it("renders the previous Talaria wallet panel copy", () => {
    render(<IntuitionWalletPanel />);
    expect(screen.getByText("Your wallet")).toBeTruthy();
    expect(screen.getByRole("button", { name: /connect wallet/i })).toBeTruthy();
  });
});
