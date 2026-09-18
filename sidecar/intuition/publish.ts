// sidecar/intuition/publish.ts
// Pin then settle a modeled debate on Intuition testnet (protocol 3.x).

import {
  multiVaultCreateAtoms,
  multiVaultCreateTriples,
  multiVaultGetAtomCost,
  multiVaultGetTripleCost,
  multiVaultIsTermCreated,
  type WriteConfig,
} from "@0xintuition/protocol";
import {
  getMultiVaultAddressFromChainId,
  intuitionTestnet,
} from "@0xintuition/deployments";
import { calculateAtomId, calculateTripleId, createPredicateAtomData } from "@0xintuition/ids";
import { PREDICATE_DEFS } from "@0xintuition/predicates";
import { modelDebate, PREDICATE_ATOM_DATA, type DebateFact, type DebateGraph } from "./model.ts";
import {
  createPublicClient,
  createWalletClient,
  http,
  toHex,
  type PublicClient,
} from "viem";
import { privateKeyToAccount } from "viem/accounts";
import { atomDataFromPinUri, pinThing } from "./pin.ts";

const DEFAULT_RPC = "https://testnet.rpc.intuition.systems/http";
const DEFAULT_RPC_FALLBACKS = ["https://rpc.intuition-testnet.rockx.com"];

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

async function createConfig(
  privateKey: `0x${string}`,
): Promise<{ config: WriteConfig; rpcUrl: string; chainId: number }> {
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

async function ensureAtom(
  config: WriteConfig,
  atomData: string,
): Promise<{ termId: `0x${string}`; created: boolean; txHash?: `0x${string}` }> {
  const termId = calculateAtomId(atomData);
  const exists = await multiVaultIsTermCreated(config, { args: [termId] });
  if (exists) return { termId, created: false };
  const atomCost = await multiVaultGetAtomCost(config);
  const txHash = await multiVaultCreateAtoms(config, {
    args: [[toHex(atomData)], [atomCost]],
    value: atomCost,
  });
  await waitForReceipt(config.publicClient, txHash);
  return { termId, created: true, txHash };
}

async function ensureTriple(
  config: WriteConfig,
  subject: `0x${string}`,
  predicate: `0x${string}`,
  object: `0x${string}`,
): Promise<{ termId: `0x${string}`; created: boolean; txHash?: `0x${string}` }> {
  const termId = calculateTripleId(subject, predicate, object);
  const exists = await multiVaultIsTermCreated(config, { args: [termId] });
  if (exists) return { termId, created: false };
  const tripleCost = await multiVaultGetTripleCost(config);
  const txHash = await multiVaultCreateTriples(config, {
    args: [[subject], [predicate], [object], [tripleCost]],
    value: tripleCost,
  });
  await waitForReceipt(config.publicClient, txHash);
  return { termId, created: true, txHash };
}

export async function publishDebate(fact: DebateFact): Promise<Record<string, unknown>> {
  const graph: DebateGraph = modelDebate(fact);
  const pins: Record<string, string> = {};
  try {
    for (const atom of graph.atoms) {
      pins[atom.role] = await pinThing({
        name: atom.name,
        description: atom.classification,
        url: atom.sameAs ?? "",
      });
    }
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    return {
      status: "pin_failed",
      error: message,
      graph,
      pins,
    };
  }

  const key = process.env["INTUITION_PRIVATE_KEY"]?.trim() ?? "";
  if (!/^0x[0-9a-fA-F]{64}$/.test(key)) {
    return {
      status: "failed",
      error: "INTUITION_PRIVATE_KEY must be a 0x-prefixed 32-byte hex key",
      graph,
      pins,
    };
  }

  try {
    const { config, rpcUrl, chainId } = await createConfig(key as `0x${string}`);
    const predTerms: Record<string, `0x${string}`> = {};
    const txs: string[] = [];
    const predicateDatas: Array<[string, string]> = [
      ["hasProposition", PREDICATE_ATOM_DATA.hasProposition],
      ["about", PREDICATE_ATOM_DATA.about],
      [
        "hasCategory",
        createPredicateAtomData(
          PREDICATE_DEFS.hasCategory.name,
          PREDICATE_DEFS.hasCategory.description,
        ),
      ],
      [
        "hasTag",
        createPredicateAtomData(PREDICATE_DEFS.hasTag.name, PREDICATE_DEFS.hasTag.description),
      ],
    ];
    for (const [role, data] of predicateDatas) {
      const ensured = await ensureAtom(config, data);
      predTerms[role] = ensured.termId;
      if (ensured.txHash) txs.push(ensured.txHash);
    }
    const atomTerms: Record<string, `0x${string}`> = {};
    for (const atom of graph.atoms) {
      const uri = pins[atom.role];
      if (!uri) throw new Error(`missing pin for atom role ${atom.role}`);
      const ensured = await ensureAtom(config, atomDataFromPinUri(uri));
      if (ensured.termId !== calculateAtomId(atomDataFromPinUri(uri))) {
        throw new Error(`atom id mismatch for ${atom.role}`);
      }
      atomTerms[atom.role] = ensured.termId;
      if (ensured.txHash) txs.push(ensured.txHash);
    }
    const question = atomTerms.question;
    const proposition = atomTerms.proposition;
    if (!question || !proposition) throw new Error("missing question or proposition atom");
    const hasProposition = predTerms.hasProposition;
    if (!hasProposition) throw new Error("missing hasProposition predicate");
    const triple = await ensureTriple(config, question, hasProposition, proposition);
    if (triple.txHash) txs.push(triple.txHash);
    const extras: Array<[`0x${string}`, `0x${string}`, `0x${string}`]> = [];
    if (atomTerms.category && predTerms.hasCategory) {
      extras.push([question, predTerms.hasCategory, atomTerms.category]);
    }
    if (atomTerms.event && predTerms.about) {
      extras.push([proposition, predTerms.about, atomTerms.event]);
    }
    if (atomTerms.kind_tag && predTerms.hasTag) {
      extras.push([question, predTerms.hasTag, atomTerms.kind_tag]);
    }
    for (const [s, p, o] of extras) {
      const extra = await ensureTriple(config, s, p, o);
      if (extra.txHash) txs.push(extra.txHash);
    }
    return {
      status: "ok",
      graph,
      pins,
      network: { observedChainId: chainId, rpcUrl },
      terms: {
        questionAtom: { termId: atomTerms.question },
        mainTriple: { termId: triple.termId },
      },
      tx: { mainTriple: triple.txHash ?? txs.at(-1) },
    };
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    return {
      status: "failed",
      error: message,
      graph,
      pins,
    };
  }
}
