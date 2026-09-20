// sidecar/intuition/atom-resolver.ts
// URI → atom data → term id → exist-or-create.

import {
  multiVaultCreateAtoms,
  multiVaultGetAtomCost,
  multiVaultIsTermCreated,
  type WriteConfig,
} from "@0xintuition/protocol";
import { calculateAtomId } from "@0xintuition/ids";
import { toHex, type PublicClient } from "viem";
import { atomDataFromPinUri } from "./pin.ts";

export type EnsuredTerm = {
  termId: `0x${string}`;
  created: boolean;
  txHash?: `0x${string}`;
  verified: boolean;
  atomData: string;
};

async function waitForReceipt(
  publicClient: PublicClient,
  txHash: `0x${string}`,
): Promise<void> {
  await publicClient.waitForTransactionReceipt({
    hash: txHash,
    confirmations: 1,
    timeout: 120_000,
  });
}

export function termIdFromAtomData(atomData: string): `0x${string}` {
  return calculateAtomId(atomData);
}

export function termIdFromPinUri(uri: string): `0x${string}` {
  return termIdFromAtomData(atomDataFromPinUri(uri));
}

export async function ensureAtomFromData(
  config: WriteConfig,
  atomData: string,
): Promise<EnsuredTerm> {
  const termId = termIdFromAtomData(atomData);
  const exists = await multiVaultIsTermCreated(config, { args: [termId] });
  if (exists) {
    return { termId, created: false, verified: true, atomData };
  }
  const atomCost = await multiVaultGetAtomCost(config);
  const txHash = await multiVaultCreateAtoms(config, {
    args: [[toHex(atomData)], [atomCost]],
    value: atomCost,
  });
  await waitForReceipt(config.publicClient, txHash);
  const verified = await multiVaultIsTermCreated(config, { args: [termId] });
  if (!verified) {
    throw new Error(`atom ${termId} missing after create`);
  }
  return { termId, created: true, txHash, verified: true, atomData };
}

export async function ensureAtomFromPinUri(
  config: WriteConfig,
  uri: string,
): Promise<EnsuredTerm> {
  return ensureAtomFromData(config, atomDataFromPinUri(uri));
}
