// sidecar/intuition/cli.ts
// Sidecar entry: model (off-chain) or publish (pin + settle).

import { modelDebate, type DebateFact } from "./model.ts";
import { publishDebate } from "./publish.ts";

function parseFact(): DebateFact {
  const raw = process.argv[3];
  if (!raw) throw new Error("Missing DebateFact JSON argument");
  return JSON.parse(raw) as DebateFact;
}

async function main(): Promise<void> {
  const cmd = process.argv[2];
  const fact = parseFact();
  if (cmd === "model") {
    const graph = modelDebate(fact);
    process.stdout.write(
      JSON.stringify({ status: "ok", graph }) + "\n",
    );
    return;
  }
  if (cmd === "publish") {
    const out = await publishDebate(fact);
    process.stdout.write(JSON.stringify(out) + "\n");
    if (out.status !== "ok") process.exitCode = 1;
    return;
  }
  throw new Error("Usage: cli.ts model|publish <DebateFact JSON>");
}

main().catch((error: unknown) => {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(message + "\n");
  process.exit(1);
});
