// web/src/hooks/use-intuition-wallet.test.ts
import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useIntuitionWallet } from "./use-intuition-wallet";

vi.mock("wagmi", () => ({
  useAccount: () => ({
    address: "0xabc",
    isConnected: true,
  }),
  useChainId: () => 13579,
  usePublicClient: () => ({ kind: "public" }),
  useWalletClient: () => ({ data: { kind: "wallet" } }),
}));

describe("useIntuitionWallet", () => {
  it("exposes Wagmi account state without a parallel store", () => {
    const { result } = renderHook(() => useIntuitionWallet());
    expect(result.current.address).toBe("0xabc");
    expect(result.current.chainId).toBe(13579);
    expect(result.current.isConnected).toBe(true);
    expect(result.current.isSupportedChain).toBe(true);
    expect(result.current.publicClient).toEqual({ kind: "public" });
    expect(result.current.walletClient).toEqual({ kind: "wallet" });
    expect(localStorage.getItem("talaria-wallet-v1")).toBeNull();
  });
});
