// sidecar/intuition/transaction-simulator.ts
// Chain + cost checks before broadcast. No private key logging.

import {
  multiVaultGetAtomCost,
  multiVaultGetTripleCost,
  type WriteConfig,
} from "@0xintuition/protocol";
import {
  getMultiVaultAddressFromChainId,
  intuitionTestnet,
} from "@0xintuition/deployments";
import {
  createPublicClient,
  createWalletClient,
  http,
  type PublicClient,
} from "viem";
import { privateKeyToAccount } from "viem/accounts";

const DEFAULT_RPC = "https://testnet.rpc.intuition.systems/http";
const DEFAULT_RPC_FALLBACKS = ["https://rpc.intuition-testnet.rockx.com"];

export type ConnectedWrite = {
  config: WriteConfig;
  rpcUrl: string;
  chainId: number;
};

function rpcCandidates(): string[] {
  const explicit = process.env["INTUITION_RPC_URL"]?.trim();
  const csv =
    process.env["INTUITION_RPC_FALLBACK_URLS"]
      ?.split(",")
      .map((v) => v.trim())
      .filter(Boolean) ?? [];
  return [explicit || DEFAULT_RPC, ...csv, ...DEFAULT_RPC_FALLBACKS].filter(
    (v, i, a) => v.length > 0 && a.indexOf(v) === i,
  );
}

export async function connectWriteConfig(
  privateKey: `0x${string}`,
): Promise<ConnectedWrite> {
  const chain = intuitionTestnet;
  const account = privateKeyToAccount(privateKey);
  const address = getMultiVaultAddressFromChainId(chain.id);
  const failures: string[] = [];
  for (const rpcUrl of rpcCandidates()) {
    try {
      const publicClient = createPublicClient({
        chain,
        transport: http(rpcUrl),
      });
      const walletClient = createWalletClient({
        chain,
        transport: http(rpcUrl),
        account,
      });
      const observedChainId = await publicClient.getChainId();
      if (observedChainId !== chain.id) {
        failures.push(`${rpcUrl} (chainId=${observedChainId})`);
        continue;
      }
      return {
        config: { address, publicClient, walletClient },
        rpcUrl,
        chainId: observedChainId,
      };
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      failures.push(`${rpcUrl} (${message})`);
    }
  }
  throw new Error(`No usable Intuition RPC. Checked: ${failures.join("; ")}`);
}

export type CostPreview = {
  atomCost: bigint;
  tripleCost: bigint;
  chainId: number;
};

/** Read creation costs; callers must not broadcast if this throws. */
export async function previewCreateCosts(
  config: WriteConfig,
  chainId: number,
): Promise<CostPreview> {
  const atomCost = await multiVaultGetAtomCost(config);
  const tripleCost = await multiVaultGetTripleCost(config);
  if (atomCost <= 0n || tripleCost <= 0n) {
    throw new Error("protocol returned non-positive create cost");
  }
  return { atomCost, tripleCost, chainId };
}

export function assertChainId(
  publicClient: PublicClient,
  expected: number,
): Promise<void> {
  return publicClient.getChainId().then((id) => {
    if (id !== expected) {
      throw new Error(`chainId mismatch: expected ${expected}, got ${id}`);
    }
  });
}
