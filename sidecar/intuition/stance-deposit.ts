// sidecar/intuition/stance-deposit.ts
// Believe = deposit on vote triple; Dispute = deposit on counter-triple.
// Always previewDeposit before broadcast. Never allow minShares = 0.

import {
  multiVaultDeposit,
  multiVaultGetBondingCurveConfig,
  multiVaultGetCounterIdFromTripleId,
  multiVaultPreviewDeposit,
  multiVaultResolveDefaultCurveId,
  type WriteConfig,
} from "@0xintuition/protocol";
import type { Address } from "viem";
import { privateKeyToAccount } from "viem/accounts";
import {
  connectWriteConfig,
  assertChainId,
} from "./transaction-simulator.ts";

export type StanceSide = "believe" | "dispute";

const DEFAULT_SLIPPAGE_BPS = 100n; // 1%
const DEFAULT_ASSETS = 1_000_000_000_000_000n; // 0.001 native (demo)

export function computeMinShares(
  previewShares: bigint,
  slippageBps: bigint = DEFAULT_SLIPPAGE_BPS,
): bigint {
  if (previewShares <= 0n) {
    throw new Error("preview shares must be positive");
  }
  if (slippageBps < 0n || slippageBps >= 10_000n) {
    throw new Error("slippage bps out of range");
  }
  const haircut = (previewShares * slippageBps) / 10_000n;
  if (haircut >= previewShares) {
    throw new Error("minShares collapsed to 0 — raise assets or lower slippage");
  }
  return previewShares - haircut;
}

export async function resolveVaultTermId(
  config: WriteConfig,
  voteTripleId: `0x${string}`,
  stance: StanceSide,
): Promise<`0x${string}`> {
  if (stance === "believe") return voteTripleId;
  return multiVaultGetCounterIdFromTripleId(config, {
    args: [voteTripleId],
  });
}

export async function resolveDefaultCurveId(
  config: WriteConfig,
): Promise<bigint> {
  const bonding = await multiVaultGetBondingCurveConfig(config);
  return multiVaultResolveDefaultCurveId(bonding);
}

export type StancePreview = {
  status: "previewed";
  stance: StanceSide;
  voteTripleId: `0x${string}`;
  vaultTermId: `0x${string}`;
  curveId: string;
  assets: string;
  previewShares: string;
  assetsAfterFees: string;
  minShares: string;
  slippageBps: string;
  chainId: number;
};

export type StanceExecuted = Omit<StancePreview, "status"> & {
  status: "deposited";
  txHash: `0x${string}`;
};

export type StanceDepositFail = {
  status: "failed";
  stage: "resolve" | "simulate" | "broadcast" | "confirm";
  code: string;
  message: string;
  retryable: boolean;
};

function assetsFromEnv(): bigint {
  const raw = process.env["INTUITION_STANCE_ASSETS"]?.trim();
  if (!raw) return DEFAULT_ASSETS;
  try {
    return BigInt(raw);
  } catch {
    throw new Error("INTUITION_STANCE_ASSETS must be an integer string");
  }
}

function slippageFromEnv(): bigint {
  const raw = process.env["INTUITION_STANCE_SLIPPAGE_BPS"]?.trim();
  if (!raw) return DEFAULT_SLIPPAGE_BPS;
  return BigInt(raw);
}

export async function previewStanceDeposit(input: {
  voteTripleId: `0x${string}`;
  stance: StanceSide;
  privateKey: `0x${string}`;
  assets?: bigint;
}): Promise<StancePreview | StanceDepositFail> {
  try {
    const { config, chainId } = await connectWriteConfig(input.privateKey);
    await assertChainId(config.publicClient, chainId);
    const vaultTermId = await resolveVaultTermId(
      config,
      input.voteTripleId,
      input.stance,
    );
    const curveId = await resolveDefaultCurveId(config);
    const assets = input.assets ?? assetsFromEnv();
    if (assets <= 0n) {
      return {
        status: "failed",
        stage: "resolve",
        code: "invalid_assets",
        message: "assets must be > 0",
        retryable: false,
      };
    }
    const [previewShares, assetsAfterFees] = await multiVaultPreviewDeposit(
      config,
      { args: [vaultTermId, curveId, assets] },
    );
    const slippageBps = slippageFromEnv();
    const minShares = computeMinShares(previewShares, slippageBps);
    return {
      status: "previewed",
      stance: input.stance,
      voteTripleId: input.voteTripleId,
      vaultTermId,
      curveId: curveId.toString(),
      assets: assets.toString(),
      previewShares: previewShares.toString(),
      assetsAfterFees: assetsAfterFees.toString(),
      minShares: minShares.toString(),
      slippageBps: slippageBps.toString(),
      chainId,
    };
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    return {
      status: "failed",
      stage: "simulate",
      code: "preview_failed",
      message,
      retryable: true,
    };
  }
}

export async function executeStanceDeposit(input: {
  voteTripleId: `0x${string}`;
  stance: StanceSide;
  privateKey: `0x${string}`;
  assets?: bigint;
}): Promise<StanceExecuted | StanceDepositFail> {
  const preview = await previewStanceDeposit(input);
  if (preview.status !== "previewed") return preview;
  try {
    const { config, chainId } = await connectWriteConfig(input.privateKey);
    await assertChainId(config.publicClient, chainId);
    const account = privateKeyToAccount(input.privateKey);
    const receiver = account.address as Address;
    const curveId = BigInt(preview.curveId);
    const assets = BigInt(preview.assets);
    const minShares = BigInt(preview.minShares);
    if (minShares <= 0n) {
      return {
        status: "failed",
        stage: "broadcast",
        code: "min_shares_forbidden",
        message: "minShares = 0 is rejected",
        retryable: false,
      };
    }
    const txHash = await multiVaultDeposit(config, {
      args: [receiver, preview.vaultTermId, curveId, minShares],
      value: assets,
    });
    await config.publicClient.waitForTransactionReceipt({
      hash: txHash,
      confirmations: 1,
      timeout: 120_000,
    });
    return { ...preview, status: "deposited", txHash };
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    return {
      status: "failed",
      stage: "broadcast",
      code: "deposit_failed",
      message,
      retryable: true,
    };
  }
}
