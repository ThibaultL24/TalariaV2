// web/src/components/wallet/wallet-connect-button.test.tsx
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { WalletConnectButton } from "./wallet-connect-button";

const walletState = vi.hoisted(() => ({
  address: undefined as `0x${string}` | undefined,
  chainId: 13579,
  isConnected: false,
  isSupportedChain: true,
  isConnecting: false,
  connectError: null as Error | null,
}));

const connect = vi.fn();
const disconnect = vi.fn();
const switchChain = vi.fn();

vi.mock("@/hooks/use-intuition-wallet", () => ({
  useIntuitionWallet: () => ({
    address: walletState.address,
    chainId: walletState.chainId,
    walletClient: undefined,
    publicClient: undefined,
    isConnected: walletState.isConnected,
    isSupportedChain: walletState.isSupportedChain,
  }),
}));

vi.mock("@/hooks/use-talaria-session", () => ({
  useTalariaSession: () => ({
    authenticated: false,
    signingIn: false,
    signIn: vi.fn(),
    logout: vi.fn(),
  }),
}));

vi.mock("wagmi", () => ({
  useConnect: () => ({
    connect,
    connectors: [{ id: "injected", name: "Injected" }],
    isPending: walletState.isConnecting,
    error: walletState.connectError,
  }),
  useDisconnect: () => ({ disconnect }),
  useSwitchChain: () => ({ switchChain, isPending: false }),
}));

describe("WalletConnectButton", () => {
  beforeEach(() => {
    localStorage.clear();
    walletState.address = undefined;
    walletState.chainId = 13579;
    walletState.isConnected = false;
    walletState.isSupportedChain = true;
    walletState.isConnecting = false;
    walletState.connectError = null;
    connect.mockReset();
    disconnect.mockReset();
    switchChain.mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  it("connects via Wagmi and does not write talaria-wallet-v1", () => {
    render(<WalletConnectButton />);
    fireEvent.click(screen.getByRole("button", { name: /connect wallet/i }));
    expect(connect).toHaveBeenCalled();
    expect(localStorage.getItem("talaria-wallet-v1")).toBeNull();
  });

  it("shows the connected address and disconnects via Wagmi", () => {
    walletState.isConnected = true;
    walletState.address = "0x1234567890abcdef1234567890abcdef12345678";
    render(<WalletConnectButton />);
    expect(screen.getByTitle(walletState.address)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /disconnect/i }));
    expect(disconnect).toHaveBeenCalled();
    expect(localStorage.getItem("talaria-wallet-v1")).toBeNull();
  });

  it("prompts to switch network when the chain is unsupported", () => {
    walletState.isConnected = true;
    walletState.address = "0x1234567890abcdef1234567890abcdef12345678";
    walletState.chainId = 1;
    walletState.isSupportedChain = false;
    render(<WalletConnectButton />);
    fireEvent.click(screen.getByRole("button", { name: /switch network/i }));
    expect(switchChain).toHaveBeenCalled();
  });
});
