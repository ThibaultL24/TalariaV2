// sidecar/intuition/cli.ts
// Sidecar entry: model | publish | stance-preview | stance-deposit. JSON only.

import { modelDebate } from "./model.ts";
import { publishDebate } from "./publisher.ts";
import { parseDebateFactInput } from "./schemas.ts";
import {
  executeStanceDeposit,
  previewStanceDeposit,
  type StanceSide,
} from "./stance-deposit.ts";

function parseFact() {
  const raw = process.argv[3];
  if (!raw) throw new Error("Missing DebateFact JSON argument");
  return parseDebateFactInput(JSON.parse(raw) as unknown);
}

function parseStanceArgs(): {
  voteTripleId: `0x${string}`;
  stance: StanceSide;
} {
  const raw = process.argv[3];
  if (!raw) throw new Error("Missing stance JSON argument");
  const body = JSON.parse(raw) as {
    voteTripleId?: string;
    stance?: string;
  };
  const voteTripleId = body.voteTripleId?.trim() ?? "";
  if (!/^0x[0-9a-fA-F]{64}$/.test(voteTripleId)) {
    throw new Error("voteTripleId must be 0x-prefixed 32-byte hex");
  }
  const stanceRaw = body.stance?.trim().toLowerCase() ?? "";
  if (stanceRaw !== "believe" && stanceRaw !== "dispute") {
    throw new Error("stance must be believe|dispute");
  }
  return {
    voteTripleId: voteTripleId as `0x${string}`,
    stance: stanceRaw,
  };
}

function requirePrivateKey(): `0x${string}` {
  const key = process.env["INTUITION_PRIVATE_KEY"]?.trim() ?? "";
  if (!/^0x[0-9a-fA-F]{64}$/.test(key)) {
    throw new Error("INTUITION_PRIVATE_KEY must be a 0x-prefixed 32-byte hex key");
  }
  return key as `0x${string}`;
}

async function main(): Promise<void> {
  const cmd = process.argv[2];
  if (cmd === "model") {
    const fact = parseFact();
    const graph = modelDebate(fact);
    process.stdout.write(JSON.stringify({ status: "ok", graph }) + "\n");
    return;
  }
  if (cmd === "publish") {
    const fact = parseFact();
    const out = await publishDebate(fact);
    process.stdout.write(JSON.stringify(out) + "\n");
    if (out.status !== "published") process.exitCode = 1;
    return;
  }
  if (cmd === "stance-preview") {
    const args = parseStanceArgs();
    const out = await previewStanceDeposit({
      ...args,
      privateKey: requirePrivateKey(),
    });
    process.stdout.write(JSON.stringify(out) + "\n");
    if (out.status !== "previewed") process.exitCode = 1;
    return;
  }
  if (cmd === "stance-deposit") {
    if (process.env["INTUITION_STANCE_EXECUTE"]?.trim() !== "1") {
      process.stdout.write(
        JSON.stringify({
          status: "failed",
          stage: "broadcast",
          code: "execute_disabled",
          message:
            "Set INTUITION_STANCE_EXECUTE=1 to broadcast stance deposits (preview via stance-preview).",
          retryable: false,
        }) + "\n",
      );
      process.exitCode = 1;
      return;
    }
    const args = parseStanceArgs();
    const out = await executeStanceDeposit({
      ...args,
      privateKey: requirePrivateKey(),
    });
    process.stdout.write(JSON.stringify(out) + "\n");
    if (out.status !== "deposited") process.exitCode = 1;
    return;
  }
  throw new Error(
    "Usage: cli.ts model|publish|stance-preview|stance-deposit <JSON>",
  );
}

main().catch((error: unknown) => {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(message + "\n");
  process.exit(1);
});
