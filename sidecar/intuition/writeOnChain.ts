// sidecar/intuition/writeOnChain.ts
// Port of Talaria POC writeIntuitionOnChain — Intuition testnet only.

import {
  createAtomFromString,
  createTripleStatement,
  getMultiVaultAddressFromChainId,
  intuitionTestnet,
  multiVaultGetAtomCost,
  multiVaultGetTripleCost,
  multiVaultIsTermCreated,
  calculateAtomId,
  calculateTripleId,
} from "@0xintuition/sdk";
import { multiVaultDeposit, type WriteConfig } from "@0xintuition/protocol";
import {
  createPublicClient,
  createWalletClient,
  http,
  toHex,
  type PublicClient
} from "viem";
import type { Address } from "viem";
import { privateKeyToAccount } from "viem/accounts";
import { z } from "zod";

const payloadSchema = z.object({
  subject: z.string().min(1),
  predicate: z.string().min(1),
  object: z.string().min(1),
  positionKind: z.enum(["believe", "disbelieve", "uncertain"]),
  attentionWei: z.string().optional(),
  convictionWei: z.string().optional(),
  counterSubject: z.string().optional(),
  counterPredicate: z.string().optional(),
  counterObject: z.string().optional(),
});

const DEFAULT_RPC_URL = "https://testnet.rpc.intuition.systems/http";
const DEFAULT_RPC_FALLBACKS = ["https://rpc.intuition-testnet.rockx.com"];

function parsePayload(): z.infer<typeof payloadSchema> {
  const raw = process.argv[2];
  if (!raw) throw new Error("Missing payload JSON argument");
  const parsed = JSON.parse(raw) as unknown;
  return payloadSchema.parse(parsed);
}

function parseWei(value: string | undefined, fallback = 0n): bigint {
  if (!value) return fallback;
  const s = value.trim();
  if (s.length === 0) return fallback;
  if (!/^\d+$/.test(s)) throw new Error(`Invalid wei amount: ${value}`);
  return BigInt(s);
}

function defaultCurveId(): bigint {
  return BigInt(process.env["INTUITION_CURVE_ID"]?.trim() || "1");
}

function parsePositiveIntegerEnv(name: string, fallback: number): number {
  const raw = process.env[name]?.trim();
  if (!raw) return fallback;
  if (!/^\d+$/.test(raw)) {
    throw new Error(`${name} must be a positive integer`);
  }
  const parsed = Number(raw);
  if (!Number.isSafeInteger(parsed) || parsed < 1) {
    throw new Error(`${name} must be a positive safe integer`);
  }
  return parsed;
}

function parseRpcCandidates(): string[] {
  const explicit = process.env["INTUITION_RPC_URL"]?.trim();
  const csv =
    process.env["INTUITION_RPC_FALLBACK_URLS"]
      ?.split(",")
      .map((value) => value.trim())
      .filter((value) => value.length > 0) ?? [];

  const candidates = [explicit || DEFAULT_RPC_URL, ...csv, ...DEFAULT_RPC_FALLBACKS]
    .map((value) => value.trim())
    .filter((value, idx, arr) => value.length > 0 && arr.indexOf(value) === idx);

  if (candidates.length === 0) return [DEFAULT_RPC_URL, ...DEFAULT_RPC_FALLBACKS];
  return candidates;
}

async function createConfigWithRpcFailover(
  privateKeyRaw: `0x${string}`,
): Promise<{ config: WriteConfig; rpcUrl: string; chainId: number }> {
  const chain = intuitionTestnet;
  const account = privateKeyToAccount(privateKeyRaw);
  const address = getMultiVaultAddressFromChainId(chain.id);
  const rpcCandidates = parseRpcCandidates();

  const failures: string[] = [];
  for (const rpcUrl of rpcCandidates) {
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
        failures.push(`${rpcUrl} (chainId=${observedChainId}, expected=${chain.id})`);
        continue;
      }

      return {
        config: {
          address,
          publicClient,
          walletClient,
        } as WriteConfig,
        rpcUrl,
        chainId: observedChainId,
      };
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      failures.push(`${rpcUrl} (${message})`);
    }
  }

  throw new Error(
    `No usable Intuition RPC endpoint found. Checked: ${failures.join("; ")}`,
  );
}

async function waitForReceipt(
  publicClient: PublicClient,
  txHash: `0x${string}`,
): Promise<void> {
  const confirmations = parsePositiveIntegerEnv(
    "INTUITION_TX_CONFIRMATIONS",
    1,
  );
  const timeoutMs = parsePositiveIntegerEnv("INTUITION_TX_TIMEOUT_MS", 120_000);
  await publicClient.waitForTransactionReceipt({
    hash: txHash,
    confirmations,
    timeout: timeoutMs,
  });
}

async function ensureAtomTerm(
  phrase: string,
  config: WriteConfig,
  atomInitialDepositWei: bigint,
): Promise<{ termId: `0x${string}`; created: boolean; txHash?: `0x${string}` }> {
  const atomData = toHex(phrase);
  const termId = calculateAtomId(atomData) as `0x${string}`;
  const exists = await multiVaultIsTermCreated(config, { args: [termId] });
  if (exists) return { termId, created: false };

  const atomCost = await multiVaultGetAtomCost(config);
  const amount = atomCost + atomInitialDepositWei;
  const created = await createAtomFromString(config, phrase, amount - atomCost);
  await waitForReceipt(config.publicClient, created.transactionHash);
  return { termId, created: true, txHash: created.transactionHash };
}

async function ensureTriple(
  config: WriteConfig,
  parts: { subject: string; predicate: string; object: string },
  atomInitialDepositWei: bigint,
  tripleInitialDepositWei: bigint,
): Promise<{ tripleTermId: `0x${string}`; created: boolean; txHash?: `0x${string}` }> {
  const subject = await ensureAtomTerm(parts.subject, config, atomInitialDepositWei);
  const predicate = await ensureAtomTerm(
    parts.predicate,
    config,
    atomInitialDepositWei,
  );
  const object = await ensureAtomTerm(parts.object, config, atomInitialDepositWei);

  // Triple IDs are derived from atom termIds (bytes32), not raw string payload bytes.
  const tripleTermId = calculateTripleId(
    subject.termId,
    predicate.termId,
    object.termId,
  ) as `0x${string}`;
  const tripleExists = await multiVaultIsTermCreated(config, {
    args: [tripleTermId],
  });
  if (tripleExists) {
    return { tripleTermId, created: false };
  }

  const tripleCost = await multiVaultGetTripleCost(config);
  const assets = tripleCost + tripleInitialDepositWei;
  const triple = await createTripleStatement(config, {
    args: [[subject.termId], [predicate.termId], [object.termId], [assets]],
    value: assets,
  });
  await waitForReceipt(config.publicClient, triple.transactionHash);
  return { tripleTermId, created: true, txHash: triple.transactionHash };
}

async function depositWei(
  config: WriteConfig,
  receiver: Address,
  termId: `0x${string}`,
  wei: bigint,
): Promise<`0x${string}`> {
  if (wei <= 0n) {
    throw new Error("depositWei: wei must be positive");
  }
  const curveId = defaultCurveId();
  const minShares = 0n;
  const txHash = await multiVaultDeposit(config, {
    args: [receiver, termId, curveId, minShares],
    value: wei,
  });
  if (!txHash) throw new Error("multiVaultDeposit returned empty");
  await waitForReceipt(config.publicClient, txHash);
  return txHash;
}

async function main(): Promise<void> {
  const payload = parsePayload();

  const privateKeyRaw = process.env["INTUITION_PRIVATE_KEY"]?.trim();
  if (!privateKeyRaw) throw new Error("INTUITION_PRIVATE_KEY is required");
  if (!/^0x[0-9a-fA-F]{64}$/.test(privateKeyRaw)) {
    throw new Error("INTUITION_PRIVATE_KEY must be a 32-byte hex key");
  }

  const chain = intuitionTestnet;
  const { config, rpcUrl, chainId } = await createConfigWithRpcFailover(
    privateKeyRaw as `0x${string}`,
  );
  const receiver = config.walletClient.account?.address;
  if (!receiver) throw new Error("Wallet account is unavailable");

  const atomInitialDepositWei = parseWei(
    process.env["INTUITION_ATOM_INITIAL_DEPOSIT_WEI"],
    0n,
  );
  const tripleInitialDepositWei = parseWei(
    process.env["INTUITION_TRIPLE_INITIAL_DEPOSIT_WEI"],
    0n,
  );

  const attentionWei = parseWei(payload.attentionWei, 0n);
  const convictionWei = parseWei(payload.convictionWei, 0n);

  const main = await ensureTriple(
    config,
    {
      subject: payload.subject,
      predicate: payload.predicate,
      object: payload.object,
    },
    atomInitialDepositWei,
    tripleInitialDepositWei,
  );

  const questionAtom = await ensureAtomTerm(
    payload.subject,
    config,
    atomInitialDepositWei,
  );

  let counter: Awaited<ReturnType<typeof ensureTriple>> | undefined;
  const hasCounter =
    Boolean(payload.counterSubject?.trim()) &&
    Boolean(payload.counterPredicate?.trim()) &&
    Boolean(payload.counterObject?.trim());

  if (hasCounter) {
    counter = await ensureTriple(
      config,
      {
        subject: payload.counterSubject!,
        predicate: payload.counterPredicate!,
        object: payload.counterObject!,
      },
      atomInitialDepositWei,
      tripleInitialDepositWei,
    );
  }

  let attentionTx: `0x${string}` | null = null;
  if (attentionWei > 0n) {
    attentionTx = await depositWei(config, receiver, questionAtom.termId, attentionWei);
  }

  let convictionTx: `0x${string}` | null = null;
  let convictionTermId: `0x${string}` | null = null;
  let convictionSide: "main" | "counter" | "none" = "none";

  if (convictionWei > 0n && payload.positionKind !== "uncertain") {
    if (payload.positionKind === "believe") {
      convictionTermId = main.tripleTermId;
      convictionSide = "main";
      convictionTx = await depositWei(config, receiver, main.tripleTermId, convictionWei);
    } else if (payload.positionKind === "disbelieve") {
      if (counter) {
        convictionTermId = counter.tripleTermId;
        convictionSide = "counter";
        convictionTx = await depositWei(
          config,
          receiver,
          counter.tripleTermId,
          convictionWei,
        );
      }
    }
  }

  const out = {
    status: "ok" as const,
    network: { chainId: chain.id, observedChainId: chainId, rpcUrl },
    positionKind: payload.positionKind,
    terms: {
      questionAtom: { termId: questionAtom.termId },
      mainTriple: {
        termId: main.tripleTermId,
        created: main.created,
      },
      counterTriple: counter
        ? { termId: counter.tripleTermId, created: counter.created }
        : null,
    },
    conviction: {
      termId: convictionTermId,
      side: convictionSide,
    },
    tx: {
      subjectAtom: questionAtom.txHash ?? null,
      predicateAtom: null,
      objectAtom: null,
      mainTriple: main.txHash ?? null,
      counterTriple: counter?.txHash ?? null,
      attention: attentionTx,
      conviction: convictionTx,
    },
  };

  process.stdout.write(JSON.stringify(out));
}

main().catch((error: unknown) => {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(JSON.stringify({ status: "error", message }));
  process.exitCode = 1;
});
