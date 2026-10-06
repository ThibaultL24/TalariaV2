// web/src/lib/intuition/config.test.ts
import { describe, expect, it } from "vitest";
import {
  INTUITION_MAINNET_CHAIN_ID,
  INTUITION_TESTNET_CHAIN_ID,
  getIntuitionApiUrl,
  getIntuitionApiUrlForChain,
  getIntuitionChain,
  getIntuitionMultiVaultAddress,
  isSupportedIntuitionChain,
} from "./config";

describe("Intuition network config", () => {
  it("exposes testnet chain 13579 and official RPC", () => {
    const chain = getIntuitionChain("testnet");
    expect(chain.id).toBe(INTUITION_TESTNET_CHAIN_ID);
    expect(chain.id).toBe(13579);
    expect(chain.rpcUrls.default.http[0]).toContain(
      "https://testnet.rpc.intuition.systems",
    );
  });

  it("exposes mainnet chain 1155 and official RPC", () => {
    const chain = getIntuitionChain("mainnet");
    expect(chain.id).toBe(INTUITION_MAINNET_CHAIN_ID);
    expect(chain.id).toBe(1155);
    expect(chain.rpcUrls.default.http[0]).toContain(
      "https://rpc.intuition.systems",
    );
  });

  it("maps testnet GraphQL to the official testnet API", () => {
    const url = getIntuitionApiUrl("testnet");
    expect(url).toContain("testnet.intuition.sh");
    expect(url).not.toContain("mainnet.intuition.sh");
  });

  it("maps mainnet GraphQL to the official mainnet API", () => {
    const url = getIntuitionApiUrl("mainnet");
    expect(url).toContain("mainnet.intuition.sh");
    expect(url).not.toContain("testnet.intuition.sh");
  });

  it("maps chainId 13579 → testnet GraphQL and 1155 → mainnet GraphQL", () => {
    expect(getIntuitionApiUrlForChain(13579)).toBe(getIntuitionApiUrl("testnet"));
    expect(getIntuitionApiUrlForChain(1155)).toBe(getIntuitionApiUrl("mainnet"));
  });

  it("rejects unsupported chains instead of falling back to mainnet", () => {
    expect(isSupportedIntuitionChain(1)).toBe(false);
    expect(isSupportedIntuitionChain(8453)).toBe(false);
    expect(() => getIntuitionApiUrlForChain(1)).toThrow(/unsupported/i);
    expect(() => getIntuitionMultiVaultAddress(1)).toThrow(/unsupported/i);
  });

  it("resolves MultiVault via the official chain-id helper", () => {
    const testnet = getIntuitionMultiVaultAddress(13579);
    const mainnet = getIntuitionMultiVaultAddress(1155);
    expect(testnet).toMatch(/^0x[0-9a-fA-F]{40}$/);
    expect(mainnet).toMatch(/^0x[0-9a-fA-F]{40}$/);
    expect(testnet).not.toBe(mainnet);
  });
});
