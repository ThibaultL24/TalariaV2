// web/src/lib/intuition/signals.ts
// User-signed MultiVault deposits. protocol@2.0.3 via sdk@3.0.1.
// minShares is never 0. Slippage is centralized in DEFAULT_SIGNAL_SLIPPAGE_BPS (100 = 1%).

import {
  getMultiVaultAddressFromChainId,
  multiVaultDeposit,
  multiVaultGetBondingCurveConfig,
  multiVaultGetInverseTripleId,
  multiVaultPreviewDeposit,
  type WriteConfig,
} from "@0xintuition/protocol";
import { parseEther, formatEther, type PublicClient, type WalletClient } from "viem";
import { getIntuitionMultiVaultAddress } from "./config";

export const DEFAULT_SIGNAL_SLIPPAGE_BPS = 100n;
export const TRUST_PRESETS = ["1", "5", "10"] as const;

export type ClaimSignalAction = "support" | "dispute";

export type SignalClients = {
  publicClient: PublicClient;
  walletClient: WalletClient;
  address: `0x${string}`;
  chainId: number;
};

export function computeMinShares(
  previewShares: bigint,
  slippageBps: bigint = DEFAULT_SIGNAL_SLIPPAGE_BPS,
): bigint {
  if (previewShares <= 0n) {
    throw new Error("preview shares must be positive");
  }
  if (slippageBps < 0n || slippageBps >= 10_000n) {
    throw new Error("slippage bps out of range");
  }
  const minShares = (previewShares * (10_000n - slippageBps)) / 10_000n;
  if (minShares <= 0n) {
    throw new Error("minShares collapsed to 0 — raise assets or lower slippage");
  }
  return minShares;
}

export function parseTrustAmount(raw: string): bigint {
  const trimmed = raw.trim();
  if (!trimmed) throw new Error("amount required");
  const assets = parseEther(trimmed);
  if (assets <= 0n) throw new Error("amount must be greater than 0");
  return assets;
}

export function formatTrustAmount(assets: bigint): string {
  return formatEther(assets);
}

function writeConfig(clients: SignalClients): WriteConfig {
  return {
    address: getIntuitionMultiVaultAddress(clients.chainId),
    publicClient: clients.publicClient,
    walletClient: clients.walletClient,
  };
}

export async function resolveClaimSignalTarget(
  clients: SignalClients,
  tripleTermId: `0x${string}`,
  action: ClaimSignalAction,
): Promise<`0x${string}`> {
  if (action === "support") return tripleTermId;
  const config = writeConfig(clients);
  return multiVaultGetInverseTripleId(config, { args: [tripleTermId] });
}

export async function resolveDefaultCurveId(clients: SignalClients): Promise<bigint> {
  const bonding = await multiVaultGetBondingCurveConfig(writeConfig(clients));
  return bonding.defaultCurveId;
}

export type ClaimSignalPreview = {
  action: ClaimSignalAction;
  tripleTermId: `0x${string}`;
  vaultTermId: `0x${string}`;
  curveId: bigint;
  assets: bigint;
  previewShares: bigint;
  assetsAfterFees: bigint;
  minShares: bigint;
  slippageBps: bigint;
  chainId: number;
  multiVault: `0x${string}`;
};

export async function previewClaimSignal(input: {
  clients: SignalClients;
  tripleTermId: `0x${string}`;
  action: ClaimSignalAction;
  assets: bigint;
}): Promise<ClaimSignalPreview> {
  const vaultTermId = await resolveClaimSignalTarget(
    input.clients,
    input.tripleTermId,
    input.action,
  );
  const curveId = await resolveDefaultCurveId(input.clients);
  const config = writeConfig(input.clients);
  const [previewShares, assetsAfterFees] = await multiVaultPreviewDeposit(config, {
    args: [vaultTermId, curveId, input.assets],
  });
  const minShares = computeMinShares(previewShares);
  return {
    action: input.action,
    tripleTermId: input.tripleTermId,
    vaultTermId,
    curveId,
    assets: input.assets,
    previewShares,
    assetsAfterFees,
    minShares,
    slippageBps: DEFAULT_SIGNAL_SLIPPAGE_BPS,
    chainId: input.clients.chainId,
    multiVault: getMultiVaultAddressFromChainId(input.clients.chainId),
  };
}

export async function depositClaimSignal(input: {
  clients: SignalClients;
  preview: ClaimSignalPreview;
}): Promise<`0x${string}`> {
  if (input.preview.minShares <= 0n) {
    throw new Error("minShares = 0 is rejected");
  }
  const config = writeConfig(input.clients);
  return multiVaultDeposit(config, {
    args: [
      input.clients.address,
      input.preview.vaultTermId,
      input.preview.curveId,
      input.preview.minShares,
    ],
    value: input.preview.assets,
  });
}
