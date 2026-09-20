// sidecar/intuition/triple-resolver.ts
// Subject/predicate/object term ids → triple term id → exist-or-create.

import {
  multiVaultCreateTriples,
  multiVaultGetTripleCost,
  multiVaultIsTermCreated,
  type WriteConfig,
} from "@0xintuition/protocol";
import {
  calculateCounterTripleId,
  calculateTripleId,
} from "@0xintuition/ids";
import type { PublicClient } from "viem";

export type EnsuredTriple = {
  termId: `0x${string}`;
  created: boolean;
  txHash?: `0x${string}`;
  verified: boolean;
  counterTermId: `0x${string}`;
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

export function voteTripleId(
  subject: `0x${string}`,
  predicate: `0x${string}`,
  object: `0x${string}`,
): `0x${string}` {
  return calculateTripleId(subject, predicate, object);
}

/** Native Intuition dispute target — never a second fake proposition. */
export function disputeCounterTripleId(
  voteTriple: `0x${string}`,
): `0x${string}` {
  return calculateCounterTripleId(voteTriple);
}

export async function ensureTriple(
  config: WriteConfig,
  subject: `0x${string}`,
  predicate: `0x${string}`,
  object: `0x${string}`,
): Promise<EnsuredTriple> {
  const termId = voteTripleId(subject, predicate, object);
  const counterTermId = disputeCounterTripleId(termId);
  const exists = await multiVaultIsTermCreated(config, { args: [termId] });
  if (exists) {
    return { termId, created: false, verified: true, counterTermId };
  }
  const tripleCost = await multiVaultGetTripleCost(config);
  const txHash = await multiVaultCreateTriples(config, {
    args: [[subject], [predicate], [object], [tripleCost]],
    value: tripleCost,
  });
  await waitForReceipt(config.publicClient, txHash);
  const verified = await multiVaultIsTermCreated(config, { args: [termId] });
  if (!verified) {
    throw new Error(`triple ${termId} missing after create`);
  }
  return { termId, created: true, txHash, verified: true, counterTermId };
}
