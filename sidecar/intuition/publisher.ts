// sidecar/intuition/publisher.ts
// Orchestrate: model → pin → resolve predicates/atoms/triples → structured result.

import type { DebateFact } from "./schemas.ts";
import { modelDebate, type DebateGraph } from "./model.ts";
import { pinThing } from "./pin.ts";
import { predicateAtomData, type PredicateKey } from "./predicates.ts";
import { ensureAtomFromData, ensureAtomFromPinUri } from "./atom-resolver.ts";
import { ensureTriple } from "./triple-resolver.ts";
import {
  connectWriteConfig,
  previewCreateCosts,
} from "./transaction-simulator.ts";

export type PublishAtomResult = {
  role: string;
  termId: `0x${string}`;
  ipfsUri: string;
  created: boolean;
  verified: boolean;
};

export type PublishTripleResult = {
  role: string;
  termId: `0x${string}`;
  txHash?: `0x${string}`;
  created: boolean;
  verified: boolean;
  counterTermId?: `0x${string}`;
};

export type PublishOk = {
  status: "published";
  chainId: number;
  graph: DebateGraph;
  atoms: PublishAtomResult[];
  triples: PublishTripleResult[];
  pins: Record<string, string>;
};

export type PublishFail = {
  status: "failed" | "pin_failed";
  stage: "pin" | "resolve" | "simulate" | "broadcast" | "confirm" | "verify" | "persist";
  code: string;
  message: string;
  retryable: boolean;
  graph?: DebateGraph;
  pins?: Record<string, string>;
};

export type PublishResult = PublishOk | PublishFail;

function fail(
  stage: PublishFail["stage"],
  code: string,
  message: string,
  retryable: boolean,
  extra?: Partial<PublishFail>,
): PublishFail {
  return {
    status: stage === "pin" ? "pin_failed" : "failed",
    stage,
    code,
    message,
    retryable,
    ...extra,
  };
}

export async function publishDebate(fact: DebateFact): Promise<PublishResult> {
  const graph = modelDebate(fact);
  const pins: Record<string, string> = {};
  try {
    for (const atom of graph.atoms) {
      pins[atom.role] = await pinThing({
        name: atom.pin.name,
        description: atom.pin.description,
        url: atom.pin.url,
      });
    }
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    return fail("pin", "pin_failed", message, true, { graph, pins });
  }

  const key = process.env["INTUITION_PRIVATE_KEY"]?.trim() ?? "";
  if (!/^0x[0-9a-fA-F]{64}$/.test(key)) {
    return fail(
      "resolve",
      "missing_private_key",
      "INTUITION_PRIVATE_KEY must be a 0x-prefixed 32-byte hex key",
      false,
      { graph, pins },
    );
  }

  try {
    const { config, chainId } = await connectWriteConfig(key as `0x${string}`);
    await previewCreateCosts(config, chainId);

    const predKeys = [
      ...new Set(graph.triples.map((t) => t.predicateKey)),
    ] as PredicateKey[];
    const predTerms: Partial<Record<PredicateKey, `0x${string}`>> = {};
    for (const keyName of predKeys) {
      const ensured = await ensureAtomFromData(config, predicateAtomData(keyName));
      predTerms[keyName] = ensured.termId;
    }

    const atomResults: PublishAtomResult[] = [];
    const atomTerms: Record<string, `0x${string}`> = {};
    for (const atom of graph.atoms) {
      const uri = pins[atom.role];
      if (!uri) {
        return fail(
          "resolve",
          "missing_pin",
          `missing pin for atom role ${atom.role}`,
          true,
          { graph, pins },
        );
      }
      const ensured = await ensureAtomFromPinUri(config, uri);
      atomTerms[atom.role] = ensured.termId;
      atomResults.push({
        role: atom.role,
        termId: ensured.termId,
        ipfsUri: uri,
        created: ensured.created,
        verified: ensured.verified,
      });
    }

    const tripleResults: PublishTripleResult[] = [];
    for (const triple of graph.triples) {
      const subject = atomTerms[triple.subjectRole];
      const object = atomTerms[triple.objectRole];
      const predicate = predTerms[triple.predicateKey];
      if (!subject || !object || !predicate) {
        return fail(
          "resolve",
          "missing_term",
          `unresolved terms for triple ${triple.role}`,
          true,
          { graph, pins },
        );
      }
      const ensured = await ensureTriple(config, subject, predicate, object);
      tripleResults.push({
        role: triple.role,
        termId: ensured.termId,
        txHash: ensured.txHash,
        created: ensured.created,
        verified: ensured.verified,
        counterTermId:
          triple.role === "question_has_proposition"
            ? ensured.counterTermId
            : undefined,
      });
    }

    return {
      status: "published",
      chainId,
      graph,
      atoms: atomResults,
      triples: tripleResults,
      pins,
    };
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    return fail("broadcast", "publish_failed", message, true, { graph, pins });
  }
}
